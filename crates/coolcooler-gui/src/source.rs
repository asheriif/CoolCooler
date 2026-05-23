use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::{AnimationDecoder, RgbaImage};

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub(crate) struct SourceState {
    path: Option<PathBuf>,
    frames: Vec<SourceFrame>,
    filename: String,
    loading: bool,
    current_frame: usize,
    last_advance: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceSummary {
    pub(crate) filename: String,
    pub(crate) frame_count: usize,
}

impl SourceState {
    pub(crate) fn new() -> Self {
        Self {
            path: None,
            frames: Vec::new(),
            filename: String::new(),
            loading: false,
            current_frame: 0,
            last_advance: Instant::now(),
        }
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub(crate) fn current_size(&self) -> Option<(u32, u32)> {
        self.current_frame()
            .map(|src| (src.rgba.width(), src.rgba.height()))
    }

    pub(crate) fn current_frame(&self) -> Option<&SourceFrame> {
        self.frames.get(self.current_frame)
    }

    pub(crate) fn begin_loading(&mut self, path: PathBuf) -> String {
        let filename = filename_for_path(&path);
        self.path = Some(path);
        self.frames.clear();
        self.filename = filename.clone();
        self.loading = true;
        self.reset_animation();
        filename
    }

    pub(crate) fn replace_with_loaded(
        &mut self,
        data: LoadedData,
        path: Option<PathBuf>,
    ) -> SourceSummary {
        let frame_count = data.frame_count();
        let (frames, filename) = data.into_parts();
        self.path = path;
        self.frames = frames;
        self.filename = filename.clone();
        self.loading = false;
        self.reset_animation();
        SourceSummary {
            filename,
            frame_count,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.path = None;
        self.frames.clear();
        self.filename.clear();
        self.loading = false;
        self.reset_animation();
    }

    pub(crate) fn advance_frame_if_due(&mut self) -> bool {
        if !self.is_animated() {
            return false;
        }

        let dur = self.frames[self.current_frame].duration;
        if self.last_advance.elapsed() < dur {
            return false;
        }

        self.current_frame = (self.current_frame + 1) % self.frames.len();
        self.last_advance = Instant::now();
        true
    }

    fn reset_animation(&mut self) {
        self.current_frame = 0;
        self.last_advance = Instant::now();
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

pub(crate) fn filename_for_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn loaded_data(filename: &str) -> LoadedData {
        LoadedData {
            frames: Arc::new(vec![SourceFrame {
                rgba: RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])),
                duration: Duration::from_millis(100),
            }]),
            filename: filename.to_string(),
        }
    }

    #[test]
    fn clear_removes_source_path_frames_and_loading_together() {
        let mut source = SourceState::new();
        source.begin_loading(PathBuf::from("/tmp/demo.png"));
        source.replace_with_loaded(
            loaded_data("demo.png"),
            Some(PathBuf::from("/tmp/demo.png")),
        );

        source.clear();

        assert!(source.path().is_none());
        assert_eq!(source.frame_count(), 0);
        assert!(source.current_frame().is_none());
        assert!(!source.is_loading());
    }

    #[test]
    fn loaded_source_keeps_summary_and_path_in_one_state_object() {
        let mut source = SourceState::new();
        let summary = source.replace_with_loaded(
            loaded_data("demo.png"),
            Some(PathBuf::from("/tmp/demo.png")),
        );

        assert_eq!(summary.filename, "demo.png");
        assert_eq!(summary.frame_count, 1);
        assert_eq!(source.path(), Some(Path::new("/tmp/demo.png")));
        assert_eq!(source.current_size(), Some((4, 4)));
    }
}
