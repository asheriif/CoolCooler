use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

pub(super) fn unique_staging_dir(parent: &Path, folder_name: &str) -> PathBuf {
    parent.join(format!(".{folder_name}.staging.{}", unique_suffix()))
}

pub(super) fn backup_dir_for(preset_dir: &Path) -> PathBuf {
    preset_dir.with_file_name(format!(
        ".{}.backup.{}",
        preset_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("preset"),
        unique_suffix()
    ))
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}.{}", std::process::id(), nanos)
}

/// Best-effort recovery for save staging folders left by an interrupted app run.
pub fn cleanup_stale_internal_dirs(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_folder_names_map_back_to_live_folder() {
        assert_eq!(
            internal_folder_parts(".demo.staging.123").map(|(folder, _)| folder),
            Some("demo")
        );
        assert_eq!(
            internal_folder_parts(".demo.backup.123").map(|(folder, _)| folder),
            Some("demo")
        );
        assert_eq!(internal_folder_parts("demo"), None);
    }

    #[test]
    fn cleanup_prefers_staging_over_backup_when_live_folder_is_missing() {
        let dir = temp_test_dir("staging-over-backup");
        fs::create_dir_all(dir.join(".demo.backup.1")).unwrap();
        fs::write(dir.join(".demo.backup.1").join("preset.json"), "old").unwrap();
        fs::create_dir_all(dir.join(".demo.staging.2")).unwrap();
        fs::write(dir.join(".demo.staging.2").join("preset.json"), "new").unwrap();

        cleanup_stale_internal_dirs(&dir);

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
