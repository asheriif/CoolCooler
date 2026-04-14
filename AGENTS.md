# Repository Guidelines

## Project Structure & Module Organization

This is a Rust workspace (`Cargo.toml`) with one crate per major responsibility under `crates/`. `coolcooler-core` owns shared traits, frame preparation, device types, and errors. `coolcooler-idcooling` implements the ID-Cooling protocol and USB device layer. `coolcooler-driver` handles detection and display loops. `coolcooler-liquidctl` builds liquidctl arguments. `coolcooler-cli` provides the `coolcooler` test CLI, and `coolcooler-gui` contains the iced app, canvas, presets, tray code, and widgets. Bundled fonts live in `assets/fonts/`. Treat `investigation/` as research material, not production source.

## Build, Test, and Development Commands

- `cargo build --workspace`: compile all crates.
- `cargo test --workspace`: run all unit and integration tests.
- `cargo fmt --all --check`: verify Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings`: run lint checks before submitting changes.
- `cargo run -p coolcooler-gui`: launch the GUI locally.
- `cargo run -p coolcooler-cli -- test`: run the CLI test command against detected hardware.
- `packaging/appimage/build-appimage-in-distrobox.sh`: enter `CoolCoolerAppImage`, build the release GUI, and place the AppImage in `dist/`.

Use `distrobox enter IsolatedArch -- cargo ...` for development. AppImages build in Ubuntu 22.04 `CoolCoolerAppImage` with home `/home/asheriif/distroboxes/CoolCoolerAppImageHome`.

## Coding Style & Naming Conventions

Use Rust 2021 idioms and standard `rustfmt` formatting. Keep crate boundaries clear: protocol construction belongs in protocol modules, shared image/device abstractions in `coolcooler-core`, and UI state or widget behavior in `coolcooler-gui`. Use `snake_case` for functions, modules, and test names; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Prefer typed errors and existing abstractions over ad hoc strings.

## Testing Guidelines

Place integration tests in `crates/<crate>/tests/` and unit tests beside the code they exercise. Name tests after behavior, for example `all_registry_commands_are_well_formed`. Run `cargo test --workspace` for logic changes; use targeted tests such as `cargo test -p coolcooler-driver` while iterating. Hardware changes need a manual CLI or GUI smoke test when a supported cooler is available.

## Commit & Pull Request Guidelines

History uses concise, imperative, sentence-case subjects such as `Add system tray icon with close-to-tray and single-instance guard`. Keep subjects focused. Pull requests should describe the change, list test commands, call out hardware coverage or gaps, and include screenshots or recordings for GUI changes.

## Agent-Specific Instructions

Do not revert unrelated local edits. Keep changes scoped, avoid modifying `Cargo.lock` unless dependency resolution requires it, and update this guide when crate layout or commands change. AppImage work belongs under `packaging/appimage/`, uses `assets/icon.png` for desktop/window identity, and keeps presets in XDG data storage.
