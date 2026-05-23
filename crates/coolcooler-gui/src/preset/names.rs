/// Sanitize a display name to a safe folder name.
pub(super) fn sanitize_folder_name(name: &str) -> String {
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

pub(super) fn is_valid_preset_folder(folder: &str) -> bool {
    is_safe_path_segment(folder) && !folder.starts_with('.')
}

pub(super) fn is_valid_preset_asset(asset: &str) -> bool {
    is_safe_path_segment(asset) && !asset.starts_with('.')
}

fn is_safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_listing_hides_hidden_and_internal_folders() {
        assert!(!is_valid_preset_folder(".demo.staging.123"));
        assert!(!is_valid_preset_folder(".demo.backup.123"));
        assert!(!is_valid_preset_folder(".hidden"));
        assert!(is_valid_preset_folder("demo"));
    }
}
