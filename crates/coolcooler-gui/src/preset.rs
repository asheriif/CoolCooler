use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use image::{codecs::png::PngEncoder, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const APP_DIR_NAME: &str = "coolcooler";
const LAST_PRESET_FILE: &str = "last_preset.json";
const STAGING_MARKER: &str = ".staging.";
const BACKUP_MARKER: &str = ".backup.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InternalPresetKind {
    Staging,
    Backup,
}

impl InternalPresetKind {
    fn recovery_priority(self) -> u8 {
        match self {
            Self::Staging => 1,
            Self::Backup => 0,
        }
    }
}

#[derive(Debug)]
struct InternalPresetDir {
    path: PathBuf,
    folder_name: String,
    kind: InternalPresetKind,
    valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LastPresetData {
    folder: String,
}

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

/// Return the last preset folder recorded by the app, if it still exists.
pub fn last_used_folder() -> Option<String> {
    let folder = read_last_used_folder()?;
    if is_hidden_or_internal_folder(&folder) {
        return None;
    }

    presets_dir()
        .join(&folder)
        .join("preset.json")
        .exists()
        .then_some(folder)
}

/// Best-effort persistence for the last preset folder.
pub fn remember_last_used(folder: &str) {
    if folder.is_empty() {
        return;
    }

    let dir = data_dir();
    let data = LastPresetData {
        folder: folder.to_string(),
    };
    let Ok(json) = serde_json::to_string_pretty(&data) else {
        return;
    };

    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(LAST_PRESET_FILE), json);
}

/// Clear the last-used pointer if it references the given preset folder.
pub fn forget_last_used_if(folder: &str) {
    if read_last_used_folder().as_deref() == Some(folder) {
        let _ = fs::remove_file(data_dir().join(LAST_PRESET_FILE));
    }
}

fn read_last_used_folder() -> Option<String> {
    let json = fs::read_to_string(data_dir().join(LAST_PRESET_FILE)).ok()?;
    let data: LastPresetData = serde_json::from_str(&json).ok()?;
    (!data.folder.is_empty()).then_some(data.folder)
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
    let staged_dir = unique_staging_dir(&dir, &folder_name);
    fs::create_dir(&staged_dir).map_err(|e| format!("Failed to create preset staging dir: {e}"))?;

    let result = write_preset_contents(&staged_dir, source_image_path, preview_rgba, data)
        .and_then(|()| install_staged_preset(&staged_dir, &preset_dir));

    if result.is_err() {
        let _ = fs::remove_dir_all(&staged_dir);
    }

    result.map(|()| folder_name)
}

fn write_preset_contents(
    preset_dir: &Path,
    source_image_path: Option<&Path>,
    preview_rgba: &RgbaImage,
    data: &PresetData,
) -> Result<(), String> {
    if let Some(src) = source_image_path {
        if src.exists() {
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let dest = preset_dir.join(format!("background.{ext}"));
            fs::copy(src, &dest).map_err(|e| format!("Failed to copy background: {e}"))?;
        }
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
    let backup_dir = preset_dir.with_file_name(format!(
        ".{}.backup.{}",
        preset_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("preset"),
        unique_suffix()
    ));

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

fn unique_staging_dir(parent: &Path, folder_name: &str) -> PathBuf {
    parent.join(format!(".{folder_name}.staging.{}", unique_suffix()))
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}.{}", std::process::id(), nanos)
}

/// Best-effort recovery for save staging folders left by an interrupted app run.
pub fn cleanup_stale_internal_dirs() {
    let dir = presets_dir();
    cleanup_stale_internal_dirs_in(&dir);
}

fn cleanup_stale_internal_dirs_in(dir: &Path) {
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    let mut internal_dirs: HashMap<String, Vec<InternalPresetDir>> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(folder) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((live_folder, kind)) = internal_folder_parts(folder) else {
            continue;
        };
        let folder_name = folder.to_string();

        internal_dirs
            .entry(live_folder.to_string())
            .or_default()
            .push(InternalPresetDir {
                valid: is_valid_preset_dir(&path),
                path,
                folder_name,
                kind,
            });
    }

    for (live_folder, candidates) in internal_dirs {
        let live_dir = dir.join(live_folder);
        if is_valid_preset_dir(&live_dir) {
            remove_internal_dirs(candidates.iter());
            continue;
        }

        if let Some(recovery_path) = recovery_candidate(&candidates).map(|c| c.path.clone()) {
            if restore_internal_dir(&recovery_path, &live_dir) {
                remove_internal_dirs(candidates.iter().filter(|c| c.path != recovery_path));
            }
        } else {
            remove_internal_dirs(candidates.iter());
        }
    }
}

fn is_hidden_or_internal_folder(folder: &str) -> bool {
    folder.starts_with('.') || is_internal_folder(folder)
}

fn is_internal_folder(folder: &str) -> bool {
    folder.starts_with('.') && (folder.contains(STAGING_MARKER) || folder.contains(BACKUP_MARKER))
}

fn live_folder_for_internal(folder: &str) -> Option<&str> {
    internal_folder_parts(folder).map(|(live_folder, _)| live_folder)
}

fn internal_folder_parts(folder: &str) -> Option<(&str, InternalPresetKind)> {
    let folder = folder.strip_prefix('.')?;
    folder
        .split_once(STAGING_MARKER)
        .map(|(live_folder, _)| (live_folder, InternalPresetKind::Staging))
        .or_else(|| {
            folder
                .split_once(BACKUP_MARKER)
                .map(|(live_folder, _)| (live_folder, InternalPresetKind::Backup))
        })
        .filter(|(live_folder, _)| !live_folder.is_empty())
}

fn is_valid_preset_dir(path: &Path) -> bool {
    path.join("preset.json").is_file()
}

fn recovery_candidate(candidates: &[InternalPresetDir]) -> Option<&InternalPresetDir> {
    candidates
        .iter()
        .filter(|candidate| candidate.valid)
        .max_by(|a, b| {
            a.kind
                .recovery_priority()
                .cmp(&b.kind.recovery_priority())
                .then_with(|| a.folder_name.cmp(&b.folder_name))
        })
}

fn restore_internal_dir(recovery_path: &Path, live_dir: &Path) -> bool {
    if live_dir.exists() && remove_path(live_dir).is_err() {
        return false;
    }

    fs::rename(recovery_path, live_dir).is_ok()
}

fn remove_internal_dirs<'a>(candidates: impl Iterator<Item = &'a InternalPresetDir>) {
    for candidate in candidates {
        let _ = fs::remove_dir_all(&candidate.path);
    }
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
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
        if is_hidden_or_internal_folder(&folder) {
            continue;
        }
        let config_path = path.join("preset.json");
        if !config_path.exists() {
            continue;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preset_listing_hides_internal_folders() {
        assert!(is_hidden_or_internal_folder(".demo.staging.123"));
        assert!(is_hidden_or_internal_folder(".demo.backup.123"));
        assert!(is_hidden_or_internal_folder(".hidden"));
        assert!(!is_hidden_or_internal_folder("demo"));
    }

    #[test]
    fn internal_folder_names_map_back_to_live_folder() {
        assert_eq!(live_folder_for_internal(".demo.staging.123"), Some("demo"));
        assert_eq!(live_folder_for_internal(".demo.backup.123"), Some("demo"));
        assert_eq!(live_folder_for_internal("demo"), None);
    }

    #[test]
    fn preset_data_keeps_widget_config_raw() {
        let data: PresetData = serde_json::from_value(json!({
            "version": 1,
            "name": "Future preset",
            "viewport": {
                "zoom": 1.0,
                "pan": [0.0, 0.0]
            },
            "widgets": [{
                "type_id": "future_widget",
                "position": [0, 0],
                "size": [10, 10],
                "opacity": 255,
                "config": {
                    "future": {
                        "nested": true
                    }
                }
            }]
        }))
        .unwrap();

        assert_eq!(
            data.widgets[0].config,
            json!({
                "future": {
                    "nested": true
                }
            })
        );
    }

    #[test]
    fn cleanup_prefers_staging_over_backup_when_live_folder_is_missing() {
        let dir = temp_test_dir("staging-over-backup");
        fs::create_dir_all(dir.join(".demo.backup.1")).unwrap();
        fs::write(dir.join(".demo.backup.1").join("preset.json"), "old").unwrap();
        fs::create_dir_all(dir.join(".demo.staging.2")).unwrap();
        fs::write(dir.join(".demo.staging.2").join("preset.json"), "new").unwrap();

        cleanup_stale_internal_dirs_in(&dir);

        assert_eq!(
            fs::read_to_string(dir.join("demo").join("preset.json")).unwrap(),
            "new"
        );
        assert!(!dir.join(".demo.backup.1").exists());
        assert!(!dir.join(".demo.staging.2").exists());

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "coolcooler-preset-{name}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
