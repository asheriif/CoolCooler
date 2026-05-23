use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::names::{is_valid_preset_asset, is_valid_preset_folder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct LastPresetData {
    pub(super) folder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PresetFolder(String);

impl PresetFolder {
    pub fn parse(folder: impl Into<String>) -> Option<Self> {
        let folder = folder.into();
        is_valid_preset_folder(&folder).then_some(Self(folder))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PresetFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PresetAsset(String);

impl PresetAsset {
    pub(super) fn parse(asset: impl Into<String>) -> Option<Self> {
        let asset = asset.into();
        is_valid_preset_asset(&asset).then_some(Self(asset))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct LoadedPreset {
    pub data: PresetData,
    pub background_path: Option<PathBuf>,
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
    pub folder: PresetFolder,
    pub preview_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn preset_folder_rejects_path_like_names() {
        assert!(PresetFolder::parse("demo").is_some());
        assert!(PresetFolder::parse("nested/demo").is_none());
        assert!(PresetFolder::parse("../demo").is_none());
        assert!(PresetFolder::parse("demo\\backup").is_none());
        assert!(PresetFolder::parse(".hidden").is_none());
        assert!(PresetFolder::parse(".demo.staging.1").is_none());
    }

    #[test]
    fn preset_asset_rejects_path_like_names() {
        assert!(PresetAsset::parse("background.png").is_some());
        assert!(PresetAsset::parse("../background.png").is_none());
        assert!(PresetAsset::parse("nested/background.png").is_none());
        assert!(PresetAsset::parse(".secret").is_none());
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
}
