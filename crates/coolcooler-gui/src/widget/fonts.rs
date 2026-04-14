//! Font registry — all available fonts for text widgets.
//!
//! Each font is embedded at compile time. Widgets store a font name (string)
//! and look up the font data at render time.

pub struct FontEntry {
    pub name: &'static str,
    pub data: &'static [u8],
}

/// All available fonts, in display order.
pub static FONTS: &[FontEntry] = &[
    FontEntry {
        name: "Inter",
        data: include_bytes!("../../../../assets/fonts/Inter.ttf"),
    },
    FontEntry {
        name: "Inter Bold",
        data: include_bytes!("../../../../assets/fonts/Inter-Bold.ttf"),
    },
    FontEntry {
        name: "Roboto",
        data: include_bytes!("../../../../assets/fonts/Roboto-Regular.ttf"),
    },
    FontEntry {
        name: "Roboto Light",
        data: include_bytes!("../../../../assets/fonts/Roboto-Light.ttf"),
    },
    FontEntry {
        name: "Roboto Bold",
        data: include_bytes!("../../../../assets/fonts/Roboto-Bold.ttf"),
    },
    FontEntry {
        name: "Arial",
        data: include_bytes!("../../../../assets/fonts/Arial.ttf"),
    },
    FontEntry {
        name: "Square721 Bold",
        data: include_bytes!("../../../../assets/fonts/Square721_BT_Bold.ttf"),
    },
    FontEntry {
        name: "Square721",
        data: include_bytes!("../../../../assets/fonts/Square721_BT_Roman.ttf"),
    },
    FontEntry {
        name: "AvantGarde",
        data: include_bytes!("../../../../assets/fonts/AvantGarde_Md_BT.TTF"),
    },
    FontEntry {
        name: "CityD Bold",
        data: include_bytes!("../../../../assets/fonts/CityDBol.ttf"),
    },
    FontEntry {
        name: "Metrofont",
        data: include_bytes!("../../../../assets/fonts/Metrofont.ttf"),
    },
    FontEntry {
        name: "Dokchamp",
        data: include_bytes!("../../../../assets/fonts/Dokchamp.ttf"),
    },
    FontEntry {
        name: "Transist",
        data: include_bytes!("../../../../assets/fonts/Transist.ttf"),
    },
    FontEntry {
        name: "Togalite Bold",
        data: include_bytes!("../../../../assets/fonts/Togalite-Bold.otf"),
    },
    FontEntry {
        name: "DigitalDream",
        data: include_bytes!("../../../../assets/fonts/DigitaldreamFat.ttf"),
    },
    FontEntry {
        name: "PixelSquare",
        data: include_bytes!("../../../../assets/fonts/PixelSquare.ttf"),
    },
    FontEntry {
        name: "Bitsumishi",
        data: include_bytes!("../../../../assets/fonts/Bitsumishi.TTF"),
    },
    FontEntry {
        name: "Balbes",
        data: include_bytes!("../../../../assets/fonts/Balbes.ttf"),
    },
    FontEntry {
        name: "Achron",
        data: include_bytes!("../../../../assets/fonts/Achron.otf"),
    },
    FontEntry {
        name: "Andes",
        data: include_bytes!("../../../../assets/fonts/Andes.ttf"),
    },
    FontEntry {
        name: "BoboBlack",
        data: include_bytes!("../../../../assets/fonts/BoboBlack.otf"),
    },
    FontEntry {
        name: "Fraiche",
        data: include_bytes!("../../../../assets/fonts/Fraiche.otf"),
    },
    FontEntry {
        name: "Kingsoft Cloud",
        data: include_bytes!("../../../../assets/fonts/Kingsoft_Cloud.ttf"),
    },
    FontEntry {
        name: "Source Han Sans",
        data: include_bytes!("../../../../assets/fonts/SourceHanSansCN-Regular.otf"),
    },
    FontEntry {
        name: "SWZ911",
        data: include_bytes!("../../../../assets/fonts/SWZ911XC.TTF"),
    },
];

/// Default font name.
pub const DEFAULT_FONT: &str = "Inter";

/// Look up font data by name. Falls back to Inter if not found.
pub fn font_data(name: &str) -> &'static [u8] {
    FONTS
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.data)
        .unwrap_or(FONTS[0].data)
}

/// All font names, for the UI dropdown.
pub fn font_names() -> Vec<&'static str> {
    FONTS.iter().map(|f| f.name).collect()
}
