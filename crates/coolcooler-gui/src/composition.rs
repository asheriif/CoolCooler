use coolcooler_driver::DisplayCapability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Static,
    Animated,
}

impl SourceKind {
    pub(crate) fn from_frame_count(frame_count: usize) -> Self {
        if frame_count > 1 {
            Self::Animated
        } else {
            Self::Static
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CanvasPolicy {
    widgets_allowed: bool,
}

impl CanvasPolicy {
    pub(crate) fn for_content(capability: Option<DisplayCapability>, source: SourceKind) -> Self {
        Self {
            widgets_allowed: widgets_allowed(capability, source),
        }
    }

    pub(crate) fn widgets_allowed(self) -> bool {
        self.widgets_allowed
    }

    pub(crate) fn widget_block_message(self) -> Option<&'static str> {
        (!self.widgets_allowed)
            .then_some("Widgets with animated backgrounds are not supported on this device.")
    }
}

fn widgets_allowed(capability: Option<DisplayCapability>, source: SourceKind) -> bool {
    !matches!(
        (capability, source),
        (Some(DisplayCapability::FileTransfer), SourceKind::Animated)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widgets_blocked_on_file_transfer_with_animated_background() {
        assert!(!CanvasPolicy::for_content(
            Some(DisplayCapability::FileTransfer),
            SourceKind::Animated
        )
        .widgets_allowed());
    }

    #[test]
    fn widgets_allowed_on_file_transfer_with_static_background() {
        assert!(CanvasPolicy::for_content(
            Some(DisplayCapability::FileTransfer),
            SourceKind::Static
        )
        .widgets_allowed());
    }

    #[test]
    fn widgets_allowed_when_device_capability_is_unknown() {
        assert!(CanvasPolicy::for_content(None, SourceKind::Animated).widgets_allowed());
    }

    #[test]
    fn widgets_allowed_on_streaming_with_animated_background() {
        assert!(CanvasPolicy::for_content(
            Some(DisplayCapability::Streaming),
            SourceKind::Animated
        )
        .widgets_allowed());
    }
}
