use std::fs;
use std::path::{Path, PathBuf};

use image::{codecs::png::PngEncoder, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const APP_DIR_NAME: &str = "coolcooler";

/// On-disk preset data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetData {
    pub version: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundData>,
    pub viewport: ViewportData,
    pub widgets: Vec<WidgetLayerData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundData {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportData {
    pub zoom: f32,
    pub pan: (f32, f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetLayerData {
    pub type_id: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub opacity: u8,
    pub config: Value,
}

/// Summary of a saved preset for the load grid.
#[derive(Debug, Clone)]
pub struct PresetEntry {
    pub name: String,
    pub folder: String,
    pub preview: Option<iced::widget::image::Handle>,
}

/// Root directory for all presets.
pub fn presets_dir() -> PathBuf {
    data_dir().join("presets")
}

/// Path to a file inside a saved preset folder.
pub fn preset_file_path(folder: &str, file: &str) -> PathBuf {
    presets_dir().join(folder).join(file)
}

fn data_dir() -> PathBuf {
    if let Some(path) = env_path("XDG_DATA_HOME") {
        return path.join(APP_DIR_NAME);
    }

    if let Some(home) = env_path("HOME") {
        return home.join(".local").join("share").join(APP_DIR_NAME);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

fn env_path(name: &str) -> Option<PathBuf> {
    let value = std::env::var_os(name)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

/// Sanitize a display name to a safe folder name.
fn sanitize_folder_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            ' ' => '-',
            _ => '_',
        })
        .collect::<String>()
        .trim_matches(|c| c == '-' || c == '_')
        .to_string()
}

/// Validate a preset name: non-empty, reasonable length, produces a valid folder name.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty");
    }
    if trimmed.len() > 64 {
        return Err("Name too long (max 64 characters)");
    }
    let folder = sanitize_folder_name(trimmed);
    if folder.is_empty() {
        return Err("Name must contain at least one letter or number");
    }
    Ok(())
}

/// Save a preset to disk.
///
/// - `name`: display name
/// - `folder_override`: if Some, overwrite this existing folder instead of creating new
/// - `source_image_path`: path to the original image/GIF file (will be copied)
/// - `preview_rgba`: current canvas composite for the thumbnail
/// - `data`: the preset configuration
pub fn save(
    name: &str,
    folder_override: Option<&str>,
    source_image_path: Option<&Path>,
    preview_rgba: &RgbaImage,
    data: &PresetData,
) -> Result<String, String> {
    let dir = presets_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create presets dir: {e}"))?;

    let folder_name = folder_override.map(|s| s.to_string()).unwrap_or_else(|| {
        let base = sanitize_folder_name(name);
        // Ensure unique folder name
        let mut candidate = base.clone();
        let mut n = 1;
        while dir.join(&candidate).exists() {
            n += 1;
            candidate = format!("{base}-{n}");
        }
        candidate
    });

    let preset_dir = dir.join(&folder_name);
    fs::create_dir_all(&preset_dir).map_err(|e| format!("Failed to create preset dir: {e}"))?;

    // Copy background image (skip if source doesn't exist or is the same file)
    if let Some(src) = source_image_path {
        if src.exists() {
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let dest = preset_dir.join(format!("background.{ext}"));
            let same_file = src
                .canonicalize()
                .and_then(|s| dest.canonicalize().map(|d| s == d))
                .unwrap_or(false);
            if !same_file {
                fs::copy(src, &dest).map_err(|e| format!("Failed to copy background: {e}"))?;
            }
        }
    }

    // Save preview thumbnail as PNG
    let preview_path = preset_dir.join("preview.png");
    let mut png_buf = Vec::new();
    PngEncoder::new(&mut png_buf)
        .write_image(
            preview_rgba.as_raw(),
            preview_rgba.width(),
            preview_rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode preview: {e}"))?;
    fs::write(&preview_path, &png_buf).map_err(|e| format!("Failed to write preview: {e}"))?;

    // Save config JSON
    let config_path = preset_dir.join("preset.json");
    let json =
        serde_json::to_string_pretty(data).map_err(|e| format!("Failed to serialize: {e}"))?;
    fs::write(&config_path, json).map_err(|e| format!("Failed to write config: {e}"))?;

    Ok(folder_name)
}

/// List all saved presets.
pub fn list() -> Vec<PresetEntry> {
    let dir = presets_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut presets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let config_path = path.join("preset.json");
        if !config_path.exists() {
            continue;
        }

        let folder = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let name = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<PresetData>(&s).ok())
            .map(|d| d.name)
            .unwrap_or_else(|| folder.clone());

        let preview_path = path.join("preview.png");
        let preview = if preview_path.exists() {
            Some(iced::widget::image::Handle::from_path(&preview_path))
        } else {
            None
        };

        presets.push(PresetEntry {
            name,
            folder,
            preview,
        });
    }

    presets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    presets
}

/// Load a preset's config from disk.
pub fn load(folder: &str) -> Result<(PresetData, Option<PathBuf>), String> {
    let preset_dir = presets_dir().join(folder);
    let config_path = preset_dir.join("preset.json");

    let json =
        fs::read_to_string(&config_path).map_err(|e| format!("Failed to read preset: {e}"))?;
    let data: PresetData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse preset: {e}"))?;

    // Find the background file
    let bg_path = data.background.as_ref().map(|bg| preset_dir.join(&bg.file));

    Ok((data, bg_path))
}

/// Delete a preset from disk.
pub fn delete(folder: &str) -> Result<(), String> {
    let preset_dir = presets_dir().join(folder);
    if preset_dir.exists() {
        fs::remove_dir_all(&preset_dir).map_err(|e| format!("Failed to delete preset: {e}"))?;
    }
    Ok(())
}
