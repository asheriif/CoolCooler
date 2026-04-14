mod driver;

use coolcooler_core::{Resolution, Rotation};

pub use driver::{build_liquidctl_args, LiquidctlDriver};

/// Definition of a liquidctl-backed LCD device.
///
/// Adding a new device = adding one entry to [`DEVICE_REGISTRY`].
pub struct LiquidctlDeviceDef {
    pub name: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub resolution: Resolution,
    pub rotation: Rotation,
    /// CLI args for `liquidctl` to send a static image.
    /// `{path}` is replaced with the image file path at runtime.
    pub set_screen_args: &'static [&'static str],
}

/// All known liquidctl-backed LCD devices.
pub static DEVICE_REGISTRY: &[LiquidctlDeviceDef] = &[
    // NZXT Kraken Z (Z53/Z63/Z73)
    LiquidctlDeviceDef {
        name: "NZXT Kraken Z",
        vendor_id: 0x1E71,
        product_id: 0x3008,
        resolution: Resolution::new(320, 320),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "static", "{path}"],
    },
    // NZXT Kraken 2023
    LiquidctlDeviceDef {
        name: "NZXT Kraken 2023",
        vendor_id: 0x1E71,
        product_id: 0x300E,
        resolution: Resolution::new(240, 240),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "static", "{path}"],
    },
    // NZXT Kraken 2023 Elite
    LiquidctlDeviceDef {
        name: "NZXT Kraken 2023 Elite",
        vendor_id: 0x1E71,
        product_id: 0x300C,
        resolution: Resolution::new(640, 640),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "static", "{path}"],
    },
    // NZXT Kraken 2024 Elite RGB
    LiquidctlDeviceDef {
        name: "NZXT Kraken 2024 Elite RGB",
        vendor_id: 0x1E71,
        product_id: 0x3012,
        resolution: Resolution::new(640, 640),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "static", "{path}"],
    },
    // NZXT Kraken 2024 Plus
    LiquidctlDeviceDef {
        name: "NZXT Kraken 2024 Plus",
        vendor_id: 0x1E71,
        product_id: 0x3014,
        resolution: Resolution::new(240, 240),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "static", "{path}"],
    },
    // MSI MPG CoreLiquid K360
    LiquidctlDeviceDef {
        name: "MSI MPG CoreLiquid K360",
        vendor_id: 0x0DB0,
        product_id: 0xB130,
        resolution: Resolution::new(240, 320),
        rotation: Rotation::None,
        set_screen_args: &["set", "lcd", "screen", "image", "1;0;{path}"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_entries() {
        assert!(
            !DEVICE_REGISTRY.is_empty(),
            "device registry should not be empty"
        );
    }

    #[test]
    fn registry_entries_have_valid_fields() {
        for def in DEVICE_REGISTRY {
            assert!(!def.name.is_empty(), "device name must not be empty");
            assert!(def.vendor_id != 0, "vendor_id must not be zero");
            assert!(def.product_id != 0, "product_id must not be zero");
            assert!(def.resolution.width > 0, "resolution width must be > 0");
            assert!(def.resolution.height > 0, "resolution height must be > 0");
            assert!(
                !def.set_screen_args.is_empty(),
                "set_screen_args must not be empty"
            );
            // Every command template must contain {path} in exactly one arg
            let path_count = def
                .set_screen_args
                .iter()
                .filter(|a| a.contains("{path}"))
                .count();
            assert_eq!(
                path_count, 1,
                "{}: set_screen_args must contain exactly one {{path}} placeholder",
                def.name
            );
        }
    }

    #[test]
    fn registry_has_no_duplicate_vid_pid() {
        for (i, a) in DEVICE_REGISTRY.iter().enumerate() {
            for (j, b) in DEVICE_REGISTRY.iter().enumerate() {
                if i != j {
                    assert!(
                        !(a.vendor_id == b.vendor_id && a.product_id == b.product_id),
                        "duplicate VID:PID {:#06x}:{:#06x} for '{}' and '{}'",
                        a.vendor_id,
                        a.product_id,
                        a.name,
                        b.name
                    );
                }
            }
        }
    }

    #[test]
    fn build_args_nzxt_kraken() {
        let def = &DEVICE_REGISTRY[0]; // NZXT Kraken Z
        let args = build_liquidctl_args(def, "/tmp/frame.png");
        assert_eq!(
            args,
            vec!["set", "lcd", "screen", "static", "/tmp/frame.png"]
        );
    }

    #[test]
    fn build_args_msi_mpg() {
        let msi = DEVICE_REGISTRY
            .iter()
            .find(|d| d.name.contains("MSI"))
            .expect("MSI device should be in registry");
        let args = build_liquidctl_args(msi, "/home/user/image.png");
        assert_eq!(
            args,
            vec!["set", "lcd", "screen", "image", "1;0;/home/user/image.png"]
        );
    }

    #[test]
    fn build_args_path_with_spaces() {
        let def = &DEVICE_REGISTRY[0];
        let args = build_liquidctl_args(def, "/tmp/my cool image.png");
        assert_eq!(
            args,
            vec!["set", "lcd", "screen", "static", "/tmp/my cool image.png"]
        );
    }

    #[test]
    fn driver_new_sets_correct_info() {
        let def = &DEVICE_REGISTRY[0]; // NZXT Kraken Z
        let driver = LiquidctlDriver::new(def);
        let info = driver.info();
        assert_eq!(info.name, "NZXT Kraken Z");
        assert_eq!(info.resolution.width, 320);
        assert_eq!(info.resolution.height, 320);
        assert_eq!(info.rotation, Rotation::None);
    }

    #[test]
    fn driver_temp_file_is_in_temp_dir() {
        let def = &DEVICE_REGISTRY[0];
        let driver = LiquidctlDriver::new(def);
        let path = driver.temp_file_path();
        assert!(
            path.starts_with(std::env::temp_dir()),
            "temp file should be in system temp dir"
        );
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "coolcooler_lcd_frame.png"
        );
    }
}
