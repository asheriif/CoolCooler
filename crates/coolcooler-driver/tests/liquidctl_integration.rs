//! Integration tests for liquidctl command building.

use coolcooler_liquidctl::{build_liquidctl_args, DEVICE_REGISTRY};

/// Verify that all registry device command templates produce well-formed args.
#[test]
fn all_registry_commands_are_well_formed() {
    for def in DEVICE_REGISTRY {
        let args = build_liquidctl_args(def, "/tmp/test.png");

        // Must start with "set"
        assert_eq!(
            args[0], "set",
            "{}: command should start with 'set'",
            def.name
        );
        // Must contain "lcd" as channel
        assert_eq!(args[1], "lcd", "{}: channel should be 'lcd'", def.name);
        // Must contain "screen" as target
        assert_eq!(args[2], "screen", "{}: target should be 'screen'", def.name);
        // Path placeholder must have been substituted
        for arg in &args {
            assert!(
                !arg.contains("{path}"),
                "{}: unsubstituted {{path}} in args: {:?}",
                def.name,
                args
            );
        }
        // The actual image path must appear somewhere in the args
        let has_path = args.iter().any(|a| a.contains("/tmp/test.png"));
        assert!(
            has_path,
            "{}: image path must appear in args: {:?}",
            def.name, args
        );
    }
}
