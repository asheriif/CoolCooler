# CoolCooler

Cross-platform GUI application for controlling LCD screens on AIO liquid coolers (starting with ID-Cooling FX360). Built in Rust with an iced GUI.

## Project Structure

Rust workspace with modular crates:

- `crates/coolcooler-core` — `CoolerLcd` trait, error types, common types (`Resolution`, `Rotation`, `DeviceInfo`), `frame::prepare` (image → JPEG pipeline), SIMD resize via `fast_image_resize`
- `crates/coolcooler-idcooling` — ID-Cooling FX360 protocol implementation (DRA/CONNECT packets) + USB device via hidapi
- `crates/coolcooler-cli` — Test CLI binary (`coolcooler test|image|color`)
- `crates/coolcooler-gui` — iced 0.14 GUI (dark/light theme, device detection, file picker, circular preview with zoom/pan, GIF animation, widget overlay system, preset save/load, quit)
- `lcd.py` — Original Python reference implementation (reverse-engineered protocol)
- Protocol docs in Obsidian: `Private/Projects/CoolCooler/Protocol.md`

## Architecture Goals

- **Multi-device support:** Abstract over different cooler brands/models via the `CoolerLcd` trait
- **Pluggable display modes:** sysinfo, now-playing, static image, GIF, solid color, etc.
- **GUI:** iced-based, with live preview, drag-and-drop image positioning, zoom controls
- **Modular:** Adding a new cooler or display mode should not require touching unrelated code

## GUI Architecture

### Widget System (`crates/coolcooler-gui/src/widget/`)
- **`LcdWidget` trait** — All canvas overlays implement this. Methods: `descriptor()` (name, category, default_size), `render(w, h, ctx)` → RgbaImage, `is_dynamic()`, `tick(ctx)`, `create_instance()`
- **`WidgetContext`** — Shared data passed to all widgets (currently carries `SysInfoData`)
- **`catalog()`** — Returns all available widget templates. Adding a widget = implement trait + add to catalog
- **Categories:** Static (Free Text, Horizontal Line, Vertical Line, Circle), Datetime (Clock, Date), System Metrics (CPU Usage, CPU Temp, RAM Usage, GPU Temp, GPU Usage)
- **`SysInfoBackend`** — Single shared backend using `sysinfo` crate, refreshed once per second. All system metrics widgets read from this shared data
- **NVML integration** — NVIDIA GPU temp/usage via `nvml-wrapper` crate (direct library call, no process spawn). Falls back gracefully if no NVIDIA GPU. AMD GPUs use sysinfo/hwmon.
- **Widget configuration** — Per-widget: text color (preset swatches), font dropdown (26 bundled fonts). Per-layer: opacity slider (0-100%)
- **Text rendering** — Multiple bundled fonts in `assets/fonts/` via `ab_glyph` + `imageproc`. Widgets render text on transparent RGBA background.
- **Font system** — `widget/fonts.rs` registers all fonts from `assets/fonts/`. Font selection per-widget via `supports_font()`/`set_font()` trait methods.

### Canvas System (`crates/coolcooler-gui/src/canvas.rs`)
- **Viewport** — Per-layer zoom (0.25x–10x) and pan state
- **LayerSelection** — Active layer: `Base` (background) or `Widget(WidgetId)`
- **WidgetLayer** — Instance on canvas: widget + position + size + visibility + opacity
- **Compositing** — `canvas.composite()` overlays all visible widgets onto base RGBA image, applying per-layer opacity

### Device Thread
- Spawned on image load, communicates via `Arc<Mutex<Vec<u8>>>` shared JPEG buffer
- UI pushes new JPEG frames after any canvas change (widget tick, animation, interaction)
- Thread sends frames at device's target FPS (20), sends keepalives every 8s

### Preset System (`crates/coolcooler-gui/src/preset.rs`)
- **Folder-per-preset:** `presets/<sanitized-name>/` with `preset.json`, background file, `preview.png`
- **Save/Save As flow:** Tracks current preset. Save updates in-place, Save As creates new
- **Widget serialization:** `type_id()`, `save_config()`, `load_config()` on `LcdWidget` trait
- **Preset grid:** Load view shows scrollable grid of preset cards with thumbnails, double-click to load, delete button

### Theme System
- **Dark/light toggle** via `iced::widget::toggler` in the title bar
- **`ThemeColors` struct** — All UI colors derived from `dark_mode: bool` via `ThemeColors::new(dark_mode)`
- **Widget styles** use `ThemeColors` methods instead of hardcoded color constants

### Subscription System
- **Animation tick** (30ms) — GIF frame advancement, only when animated source loaded
- **Widget tick** (1s) — Sysinfo refresh + dynamic widget updates, only when dynamic widgets exist

## Build & Run

Must build AND run inside distrobox (host is Bazzite, immutable — glibc mismatch prevents host execution):

```bash
distrobox enter Fedora45 -- cargo build
distrobox enter Fedora45 -- cargo test
distrobox enter Fedora45 -- ./target/debug/coolcooler test
distrobox enter Fedora45 -- ./target/debug/coolcooler-gui
```

## Key Technical Decisions

- **iced 0.14** — fixes Wayland resize lag present in 0.13
- **hidapi with libusb backend** (`linux-static-libusb`), not hidraw — the device's 1024-byte HID endpoint doesn't get a `/dev/hidraw*` node from the kernel
- **HID report ID:** Each packet write must prepend `0x00` report ID byte (handled in `Fx360::write_packet`). The protocol module produces raw 1024-byte packets; the device layer adds the HID framing
- **Protocol is transport-agnostic:** `protocol.rs` builds packets as pure functions with no USB dependency. Device crates handle the transport
- **SIMD image resize:** Uses `fast_image_resize` with CatmullRom filter (not Lanczos3 — imperceptible quality difference at 240x240, significantly faster)
- **RGBA pipeline for GIFs:** Resize in RGBA space, convert to RGB only at output resolution (240x240). Avoids expensive full-resolution RGBA→RGB conversion per frame
- **Dev profile optimization:** `opt-level = 1` for workspace code, `opt-level = 2` for dependencies. Required for acceptable image processing performance in debug builds
- **NVML for NVIDIA GPU metrics:** `nvml-wrapper` crate for GPU temp/usage. Cross-platform (Linux + Windows). `sysinfo` crate doesn't read NVIDIA sensors.
- **Framework decision:** Evaluated GTK4 and Tauri as alternatives to iced. GTK4 lacks cross-platform (Windows requires bundling GTK). Tauri uses WebKitGTK on Linux which has known rendering issues (Tauri maintainers called it "unusable"). Sticking with iced for cross-platform + native rendering.

## Conventions

- Format with `cargo fmt`
- Lint with `cargo clippy`
