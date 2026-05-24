mod names;
mod recovery;
mod schema;

use std::fs;
use std::path::{Path, PathBuf};

use image::{codecs::png::PngEncoder, ImageEncoder, RgbaImage};

pub use names::validate_name;
pub use schema::{
    BackgroundData, LoadedPreset, PresetData, PresetEntry, PresetFolder, ViewportData,
    WidgetLayerData,
};

use names::sanitize_folder_name;
use schema::{LastPresetData, PresetAsset};

const APP_DIR_NAME: &str = "coolcooler";
const LAST_PRESET_FILE: &str = "last_preset.json";

/// Root directory for all presets.
pub fn presets_dir() -> PathBuf {
    data_dir().join("presets")
}

fn preset_folder_path(folder: &PresetFolder) -> PathBuf {
    presets_dir().join(folder.as_str())
}

/// Path to a validated file inside a saved preset folder.
fn preset_file_path(folder: &PresetFolder, asset: &PresetAsset) -> PathBuf {
    preset_folder_path(folder).join(asset.as_str())
}

/// Return the last preset folder recorded by the app, if it still exists.
pub fn last_used_folder() -> Option<PresetFolder> {
    let folder = read_last_used_folder()?;
    preset_folder_path(&folder)
        .join("preset.json")
        .exists()
        .then_some(folder)
}

/// Best-effort persistence for the last preset folder.
pub fn remember_last_used(folder: &PresetFolder) {
    let dir = data_dir();
    let data = LastPresetData {
        folder: folder.as_str().to_string(),
    };
    let Ok(json) = serde_json::to_string_pretty(&data) else {
        return;
    };

    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(LAST_PRESET_FILE), json);
}

/// Clear the last-used pointer if it references the given preset folder.
pub fn forget_last_used_if(folder: &PresetFolder) {
    if read_last_used_folder().as_ref() == Some(folder) {
        let _ = fs::remove_file(data_dir().join(LAST_PRESET_FILE));
    }
}

fn read_last_used_folder() -> Option<PresetFolder> {
    let json = fs::read_to_string(data_dir().join(LAST_PRESET_FILE)).ok()?;
    let data: LastPresetData = serde_json::from_str(&json).ok()?;
    PresetFolder::parse(data.folder)
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

/// Save a preset to disk.
///
/// - `name`: display name
/// - `folder_override`: if Some, overwrite this existing folder instead of creating new
/// - `source_image_path`: path to the original image/GIF file (will be copied)
/// - `preview_rgba`: current canvas composite for the thumbnail
/// - `data`: the preset configuration
pub fn save(
    name: &str,
    folder_override: Option<&PresetFolder>,
    source_image_path: Option<&Path>,
    preview_rgba: &RgbaImage,
    data: &PresetData,
) -> Result<PresetFolder, String> {
    let dir = presets_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create presets dir: {e}"))?;

    let folder_name = folder_override
        .map(|folder| folder.as_str().to_string())
        .unwrap_or_else(|| unique_folder_name(&dir, name));
    let folder = PresetFolder::parse(folder_name.clone())
        .ok_or_else(|| "Preset folder name is not safe".to_string())?;

    let preset_dir = dir.join(&folder_name);
    let staged_dir = recovery::unique_staging_dir(&dir, &folder_name);
    fs::create_dir(&staged_dir).map_err(|e| format!("Failed to create preset staging dir: {e}"))?;

    let result = write_preset_contents(&staged_dir, source_image_path, preview_rgba, data)
        .and_then(|()| install_staged_preset(&staged_dir, &preset_dir));

    if result.is_err() {
        let _ = fs::remove_dir_all(&staged_dir);
    }

    result.map(|()| folder)
}

fn unique_folder_name(dir: &Path, name: &str) -> String {
    let base = sanitize_folder_name(name);
    let mut candidate = base.clone();
    let mut n = 1;
    while dir.join(&candidate).exists() {
        n += 1;
        candidate = format!("{base}-{n}");
    }
    candidate
}

fn write_preset_contents(
    preset_dir: &Path,
    source_image_path: Option<&Path>,
    preview_rgba: &RgbaImage,
    data: &PresetData,
) -> Result<(), String> {
    if let Some(background) = &data.background {
        let src = source_image_path
            .ok_or_else(|| "Preset has a background but no source image path".to_string())?;
        if !src.exists() {
            return Err("Background source file no longer exists".to_string());
        }
        let asset = PresetAsset::parse(background.file.clone())
            .ok_or_else(|| "Background file name is not safe".to_string())?;
        let dest = preset_dir.join(asset.as_str());
        fs::copy(src, &dest).map_err(|e| format!("Failed to copy background: {e}"))?;
    }

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

    let config_path = preset_dir.join("preset.json");
    let json =
        serde_json::to_string_pretty(data).map_err(|e| format!("Failed to serialize: {e}"))?;
    fs::write(&config_path, json).map_err(|e| format!("Failed to write config: {e}"))?;

    Ok(())
}

fn install_staged_preset(staged_dir: &Path, preset_dir: &Path) -> Result<(), String> {
    let backup_dir = recovery::backup_dir_for(preset_dir);

    if preset_dir.exists() {
        fs::rename(preset_dir, &backup_dir)
            .map_err(|e| format!("Failed to stage existing preset for replacement: {e}"))?;
    }

    if let Err(e) = fs::rename(staged_dir, preset_dir) {
        if backup_dir.exists() {
            let _ = fs::rename(&backup_dir, preset_dir);
        }
        return Err(format!("Failed to install preset: {e}"));
    }

    if backup_dir.exists() {
        let _ = fs::remove_dir_all(backup_dir);
    }

    Ok(())
}

/// Best-effort recovery for save staging folders left by an interrupted app run.
pub fn cleanup_stale_internal_dirs() {
    let dir = presets_dir();
    recovery::cleanup_stale_internal_dirs(&dir);
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
        let folder = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let Some(folder) = PresetFolder::parse(folder) else {
            continue;
        };
        let config_path = path.join("preset.json");
        if !config_path.exists() {
            continue;
        }

        let name = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<PresetData>(&s).ok())
            .map(|d| d.name)
            .unwrap_or_else(|| folder.as_str().to_string());

        let preview_path = path.join("preview.png");
        let preview_path = preview_path.exists().then_some(preview_path);

        presets.push(PresetEntry {
            name,
            folder,
            preview_path,
        });
    }

    presets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    presets
}

/// Load a preset's config from disk.
pub fn load(folder: &PresetFolder) -> Result<LoadedPreset, String> {
    let preset_dir = preset_folder_path(folder);
    let config_path = preset_dir.join("preset.json");

    let json =
        fs::read_to_string(&config_path).map_err(|e| format!("Failed to read preset: {e}"))?;
    let data: PresetData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse preset: {e}"))?;

    let background_path = data
        .background
        .as_ref()
        .map(|bg| {
            PresetAsset::parse(bg.file.clone())
                .map(|asset| preset_file_path(folder, &asset))
                .ok_or_else(|| "Preset background file name is not safe".to_string())
        })
        .transpose()?;

    Ok(LoadedPreset {
        data,
        background_path,
    })
}

/// Delete a preset from disk.
pub fn delete(folder: &PresetFolder) -> Result<(), String> {
    let preset_dir = preset_folder_path(folder);
    if preset_dir.exists() {
        fs::remove_dir_all(&preset_dir).map_err(|e| format!("Failed to delete preset: {e}"))?;
    }
    Ok(())
}
