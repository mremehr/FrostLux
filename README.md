# FrostLux

TUI controller for IKEA Tradfri smart lights. Built with Rust, ratatui, and the Frost philosophy: beautiful, fast, respects the user.

## Features

- **Vim navigation** — j/k to browse, h/l to dim, Space to toggle
- **9 scenes** — on, off, movie, bright, cozy, night, evening, reading, morning
- **Headless mode** — apply scenes from the command line without opening the TUI
- **Per-scene exclusions** — skip specific lights for specific scenes
- **Auto theme** — detects terminal light/dark mode (Ghostty, Alacritty, xterm)
- **Persistent DTLS** — single connection with auto-reconnect for fast responses
- **Optimistic UI** — instant feedback, network calls run in background

## Installation

### Requirements

- Rust toolchain (1.70+)
- OpenSSL development headers (`openssl-devel` / `libssl-dev`)
- An IKEA Tradfri gateway on your local network

### Build

```sh
cargo build --release
cp target/release/frostlux ~/.local/bin/
```

## Gateway Pairing

To control your lights, you need a PSK (pre-shared key) from the gateway.

1. Find your gateway IP (check your router or run `avahi-browse -rt _coap._udp`)
2. Find the **security code** printed on the bottom of your gateway
3. Generate credentials using `coap-client`:

```sh
coap-client -m post -u "Client_identity" -k "SECURITY_CODE" \
  -e '{"9090":"YOUR_IDENTITY"}' "coaps://GATEWAY_IP:5684/15011/9063"
```

4. The response contains your PSK. Add both to the config:

```sh
~/.config/frostlux/config.toml
```

```toml
[gateway]
host = "192.168.0.131"
identity = "YOUR_IDENTITY"
psk = "RETURNED_PSK"
```

## Usage

```sh
# Interactive TUI
frostlux

# Apply a scene directly (no TUI)
frostlux --scene movie
frostlux -s cozy
frostlux -s off
```

### Keybindings

| Key | Action |
|-----|--------|
| j / k | Navigate up/down |
| Space | Toggle on/off |
| h / l | Dim -/+ 10% |
| PgUp / PgDn | Dim -/+ 25% |
| + / - | Color temp warmer/colder |
| a / o | All on / All off |
| m / b / c | Movie / Bright / Cozy |
| n / e / r / g | Night / Evening / Reading / Morning |
| R | Force refresh |
| ? | Help |
| q | Quit |

### Scene Names

Scenes accept both English and Swedish names:

| Scene | Aliases |
|-------|---------|
| movie | film |
| bright | ljus |
| cozy | mysig |
| night | natt |
| evening | kvall |
| reading | lasning |
| morning | morgon |

## Configuration

Config lives at `~/.config/frostlux/config.toml` (auto-generated on first run).

```toml
[gateway]
host = "192.168.0.131"
identity = ""
psk = ""

[ui]
theme = "auto"           # auto, light, dark
refresh_interval = 5     # seconds

[scenes]
exclude = ["Sovrummet"]  # skip in all scenes

# skip only in specific scenes
[scenes.exclude_by_scene]
movie = ["TV-lampan"]
night = ["Koket"]
```

### Theme Detection

When `theme = "auto"`, FrostLux detects your terminal theme via:

1. `FROSTLUX_THEME` env var
2. `COLORFGBG` (xterm/rxvt/Ghostty)
3. Ghostty config
4. Alacritty marker file / config header
5. `ALACRITTY_THEME` / `GHOSTTY_THEME` env vars

Themes: **Deep Cracked Ice** (dark) and **Frostglow** (light).

## License

GPL-2.0
