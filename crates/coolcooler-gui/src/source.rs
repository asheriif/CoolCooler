use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use image::{AnimationDecoder, RgbaImage};

#[derive(Clone)]
pub(crate) struct SourceFrame {
    pub(crate) rgba: RgbaImage,
    pub(crate) duration: Duration,
}

#[derive(Clone)]
pub(crate) struct LoadedData {
    frames: Arc<Vec<SourceFrame>>,
    pub(crate) filename: String,
}

impl std::fmt::Debug for LoadedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedData")
            .field("frames", &self.frames.len())
            .field("filename", &self.filename)
            .finish()
    }
}

impl LoadedData {
    pub(crate) fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn into_parts(self) -> (Vec<SourceFrame>, String) {
        let frames = Arc::try_unwrap(self.frames).unwrap_or_else(|arc| (*arc).clone());
        (frames, self.filename)
    }
}

pub(crate) fn load_source_data(path: &Path, filename: String) -> Result<LoadedData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let frames = if ext == "gif" {
        load_gif_source_data(path)?
    } else {
        let img = image::open(path).map_err(|e| e.to_string())?;
        let rgba = img.to_rgba8();
        vec![SourceFrame {
            rgba,
            duration: Duration::MAX,
        }]
    };

    Ok(LoadedData {
        frames: Arc::new(frames),
        filename,
    })
}

fn load_gif_source_data(path: &Path) -> Result<Vec<SourceFrame>, String> {
    let file = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let decoder = image::codecs::gif::GifDecoder::new(file).map_err(|e| e.to_string())?;
    let raw_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| e.to_string())?;

    if raw_frames.is_empty() {
        return Err("GIF has no frames".to_string());
    }

    Ok(raw_frames
        .into_iter()
        .map(|raw| {
            let (n, d) = raw.delay().numer_denom_ms();
            let ms = if d == 0 { 100 } else { n / d };
            let duration = Duration::from_millis((ms).max(20) as u64);
            let rgba = raw.into_buffer();
            SourceFrame { rgba, duration }
        })
        .collect())
}
