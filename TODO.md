# CoolCooler — Planned Features

## Display Modes
- [ ] Solid color picker
- [ ] Now Playing (MPRIS/D-Bus integration)

## GUI
- [ ] Drag and drop files onto preview
- [ ] Settings/preferences panel
- [ ] System tray / background mode
- [ ] Remember last used image/settings
- [ ] Widget property editor (font size, color, format string)

## Protocol / Device
- [ ] Fan/pump control (unexplored commands)
- [ ] Sensor readback (temps, RPM via EP2 IN)
- [ ] Auto-reconnect on USB disconnect
- [ ] Multiple device support (if more than one cooler connected)

## Multi-Device
- [ ] Support for other cooler brands/models
- [ ] Device discovery / selection UI

## Packaging
- [ ] AppImage / Flatpak build
- [ ] Windows build + installer
- [ ] udev rule auto-install

## Widgets
- [ ] Controllable transparency per widget
- [ ] More system metrics (disk, network, fan speed)
- [ ] Gauge/bar visualizations (not just text)
- [ ] Custom text widget (user-defined content)
- [ ] Widget snap-to-grid / alignment helpers

## Devices
- [ ] Controllable dimensions to override defaults

## Done
- [x] Static image display
- [x] GIF animation with per-frame timing
- [x] Circular 240x240 preview matching hardware
- [x] Zoom (25%-1000%) and pan at any zoom level
- [x] Widget overlay system with layer management
- [x] Widget categories: Static, Datetime, System Metrics
- [x] Shared SysInfoBackend (CPU/GPU temp, CPU/RAM usage)
- [x] Clock and Date widgets
- [x] Separate tick rates (30ms animation, 1s sysinfo)
- [x] Live device updates for dynamic widgets
- [x] Auto-display on image select
