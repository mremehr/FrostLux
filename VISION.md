# Vision

FrostLux is a terminal-first controller for smart lights. It exists because controlling your home should be fast, beautiful, and private.

## Principles

- **Local only** — all communication stays on your network, no cloud required
- **Instant** — optimistic UI, persistent connections, zero perceived latency
- **Keyboard-driven** — Vim navigation, single-key scenes, no mouse needed
- **Respect the user** — no telemetry, no accounts, your lights your rules
- **Single binary** — one `cargo build`, one executable, no runtime dependencies

## Non-goals

- Cloud integrations or remote access
- Mobile companion app
- Voice control
- Support for non-Tradfri devices (for now)

## Roadmap

- [ ] Unit tests for scene logic, config parsing, and brightness conversion
- [ ] Light grouping (rooms)
- [ ] Custom user-defined scenes
- [ ] Scheduled scenes (wake-up light, bedtime dimming)
- [ ] Transition animations (smooth fade between brightness levels)
- [ ] Multi-gateway support
