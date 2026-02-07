# Changelog

## 0.2.0

- Per-scene light exclusions (`exclude_by_scene`)
- Headless scene mode (`frostlux --scene movie`)
- Non-blocking background refresh (UI never freezes)
- Persistent DTLS connection with auto-reconnect
- Optimistic UI updates for instant feedback
- Auto theme detection (Ghostty, Alacritty, xterm)
- Deep Cracked Ice (dark) and Frostglow (light) themes
- Gateway IP validation at config load
- Safe mutex handling (no unwrap panics)
- Release profile: LTO + strip + single codegen unit + abort on panic

## 0.1.0

- Initial prototype
- Basic TUI with light list
- Toggle on/off, brightness, color temperature
- 9 pre-defined scenes with Swedish aliases
- CoAP/DTLS communication with Tradfri gateway
