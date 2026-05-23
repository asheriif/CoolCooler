use iced::Color;

pub(crate) struct AppColors {
    pub(crate) bg: Color,
    pub(crate) card_bg: Color,
    pub(crate) accent: Color,
    pub(crate) text_dim: Color,
    pub(crate) green: Color,
    pub(crate) red: Color,
    pub(crate) surface: Color,
    pub(crate) text_primary: Color,
    pub(crate) danger_bg: Color,
    pub(crate) disabled_bg: Color,
}

pub(crate) const DARK: AppColors = AppColors {
    bg: Color::from_rgb(0.086, 0.086, 0.118),
    card_bg: Color::from_rgb(0.118, 0.118, 0.180),
    accent: Color::from_rgb(0.024, 0.714, 0.831),
    text_dim: Color::from_rgb(0.392, 0.392, 0.471),
    green: Color::from_rgb(0.133, 0.773, 0.369),
    red: Color::from_rgb(0.937, 0.267, 0.267),
    surface: Color::from_rgb(0.157, 0.157, 0.220),
    text_primary: Color::WHITE,
    danger_bg: Color::from_rgb(0.3, 0.08, 0.08),
    disabled_bg: Color::from_rgb(0.1, 0.1, 0.14),
};

pub(crate) const LIGHT: AppColors = AppColors {
    bg: Color::from_rgb(0.94, 0.94, 0.96),
    card_bg: Color::from_rgb(1.0, 1.0, 1.0),
    accent: Color::from_rgb(0.02, 0.55, 0.65),
    text_dim: Color::from_rgb(0.5, 0.5, 0.56),
    green: Color::from_rgb(0.1, 0.6, 0.3),
    red: Color::from_rgb(0.8, 0.2, 0.2),
    surface: Color::from_rgb(0.9, 0.9, 0.92),
    text_primary: Color::from_rgb(0.1, 0.1, 0.12),
    danger_bg: Color::from_rgb(1.0, 0.92, 0.92),
    disabled_bg: Color::from_rgb(0.88, 0.88, 0.90),
};
