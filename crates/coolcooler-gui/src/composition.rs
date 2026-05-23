use coolcooler_driver::{widgets_allowed, DisplayCapability};

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

    fn is_animated(self) -> bool {
        matches!(self, Self::Animated)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CanvasPolicy {
    widgets_allowed: bool,
}

impl CanvasPolicy {
    pub(crate) fn for_content(capability: Option<DisplayCapability>, source: SourceKind) -> Self {
        Self {
            widgets_allowed: capability
                .map(|capability| widgets_allowed(capability, source.is_animated()))
                .unwrap_or(true),
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
