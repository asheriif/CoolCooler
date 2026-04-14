# Repository Guidelines

## Project Structure & Module Organization

This is a Rust workspace (`Cargo.toml`) with one crate per major responsibility under `crates/`. `coolcooler-core` owns shared traits, frame preparation, device types, and errors. `coolcooler-idcooling` implements the ID-Cooling protocol and USB device layer. `coolcooler-driver` handles device detection and display-loop integration. `coolcooler-liquidctl` builds liquidctl command arguments. `coolcooler-cli` provides the `coolcooler` test CLI, and `coolcooler-gui` contains the iced desktop app, canvas, presets, tray code, and widgets. Bundled fonts live in `assets/fonts/`. `lcd.py` is a Python reference implementation; treat `investigation/` as external research material, not production source.

## Build, Test, and Development Commands

- `cargo build --workspace`: compile all crates.
- `cargo test --workspace`: run all unit and integration tests.
- `cargo fmt --all --check`: verify Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings`: run lint checks before submitting changes.
- `cargo run -p coolcooler-gui`: launch the GUI locally.
- `cargo run -p coolcooler-cli -- test`: run the CLI test command against detected hardware.

The existing notes also use `distrobox enter IsolatedArch -- cargo ...` when host dependencies should stay isolated.

## Coding Style & Naming Conventions

Use Rust 2021 idioms and standard `rustfmt` formatting with four-space indentation. Keep crate boundaries clear: protocol construction belongs in protocol modules, shared image/device abstractions in `coolcooler-core`, and UI state or widget behavior in `coolcooler-gui`. Use `snake_case` for functions, modules, and test names; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Prefer typed errors and existing abstractions over ad hoc strings.

## Testing Guidelines

Place crate-level integration tests in `crates/<crate>/tests/` and unit tests beside the code they exercise. Name tests after the behavior, for example `all_registry_commands_are_well_formed`. Run `cargo test --workspace` for logic changes; run targeted package tests such as `cargo test -p coolcooler-driver` while iterating. Hardware-facing changes should include a manual CLI or GUI smoke test when a supported cooler is available.

## Commit & Pull Request Guidelines

The current history uses concise, imperative, sentence-case commit subjects such as `Add system tray icon with close-to-tray and single-instance guard`. Keep subjects focused on the user-visible or architectural change. Pull requests should describe the change, list test commands run, call out hardware coverage or gaps, and include screenshots or short recordings for GUI changes.

## Agent-Specific Instructions

Do not revert unrelated local edits. Keep generated changes scoped, avoid modifying `Cargo.lock` unless dependency resolution requires it, and update this guide when crate layout or standard commands change.
