use ratatui::style::{Color, Modifier, Style};
use std::fs;

pub struct FrostTheme {
    pub background: Color,
    pub foreground: Color,

    pub ice_blue: Color,
    pub cold_green: Color,
    pub bright_red: Color,
    pub warm_yellow: Color,
    pub crystal_cyan: Color,

    pub border: Color,
    pub dimmed: Color,
}

impl Default for FrostTheme {
    fn default() -> Self {
        if detect_light_theme() {
            Self::frostglow_light()
        } else {
            Self::deep_cracked_ice_dark()
        }
    }
}

impl FrostTheme {
    pub fn deep_cracked_ice_dark() -> Self {
        Self {
            // Matches: ~/.config/alacritty/themes/deep-cracked-ice.toml
            background: Color::Rgb(26, 43, 56),   // #1a2b38
            foreground: Color::Rgb(240, 248, 255), // #f0f8ff

            ice_blue: Color::Rgb(126, 180, 232),   // #7eb4e8
            cold_green: Color::Rgb(111, 224, 148), // #6fe094
            bright_red: Color::Rgb(255, 107, 122), // #ff6b7a
            warm_yellow: Color::Rgb(255, 230, 128), // #ffe680
            crystal_cyan: Color::Rgb(125, 200, 245), // #7dc8f5

            border: Color::Rgb(74, 93, 115), // #4a5d73
            dimmed: Color::Rgb(74, 93, 115), // #4a5d73
        }
    }

    pub fn frostglow_light() -> Self {
        Self {
            // Matches: ~/.config/alacritty/themes/frostglow.toml
            background: Color::Rgb(240, 248, 255), // #f0f8ff
            foreground: Color::Rgb(10, 15, 20),    // #0a0f14

            ice_blue: Color::Rgb(46, 90, 144),    // #2e5a90
            cold_green: Color::Rgb(13, 117, 69),  // #0d7545
            bright_red: Color::Rgb(200, 31, 50),  // #c81f32
            warm_yellow: Color::Rgb(179, 114, 24), // #b37218
            crystal_cyan: Color::Rgb(24, 128, 176), // #1880b0

            border: Color::Rgb(184, 212, 241), // #b8d4f1
            dimmed: Color::Rgb(42, 63, 85),    // #2a3f55
        }
    }

    pub fn normal(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.ice_blue)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.foreground)
            .bg(self.border)
            .add_modifier(Modifier::BOLD)
    }
}

pub fn frost_theme_from_config(config_theme: &str) -> FrostTheme {
    match config_theme.trim().to_ascii_lowercase().as_str() {
        "light" | "frostglow" => FrostTheme::frostglow_light(),
        "dark" | "deep-cracked-ice" | "deep_cracked_ice" => FrostTheme::deep_cracked_ice_dark(),
        _ => FrostTheme::default(),
    }
}

fn parse_theme_marker(theme_marker: &str) -> Option<bool> {
    let theme = theme_marker.trim().to_lowercase();
    if theme.contains("light") || theme.contains("frostglow") {
        return Some(true);
    }
    if theme.contains("dark") || theme.contains("cracked") || theme.contains("ice") {
        return Some(false);
    }
    None
}

pub fn alacritty_marker_theme_is_light() -> Option<bool> {
    let marker_path = std::env::var("HOME")
        .ok()
        .map(|h| format!("{}/.config/alacritty/.current-theme", h))?;
    let marker = fs::read_to_string(marker_path).ok()?;
    parse_theme_marker(&marker)
}

fn detect_light_theme() -> bool {
    // 1. Check explicit FROSTLUX_THEME env var
    if let Ok(theme) = std::env::var("FROSTLUX_THEME") {
        let t = theme.to_lowercase();
        if t.contains("light") || t.contains("frostglow") {
            return true;
        }
        if t.contains("dark") {
            return false;
        }
    }

    // 2. Check Alacritty marker file first (dynamic source updated by keybindings/scripts)
    if let Some(is_light) = alacritty_marker_theme_is_light() {
        return is_light;
    }

    // 3. Check COLORFGBG (xterm, rxvt, Ghostty, etc.)
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        if let Some(bg) = colorfgbg.rsplit(';').next() {
            if let Ok(bg_idx) = bg.trim().parse::<u8>() {
                if bg_idx >= 7 {
                    return true;
                }
                return false;
            }
        }
    }

    // 4. Check Ghostty config
    if let Ok(home) = std::env::var("HOME") {
        let ghostty_config = format!("{}/.config/ghostty/config", home);
        if let Ok(content) = fs::read_to_string(&ghostty_config) {
            for line in content.lines() {
                let clean = line.split('#').next().unwrap_or("").trim();
                if clean.starts_with("theme") {
                    let lower = clean.to_lowercase();
                    if lower.contains("light") || lower.contains("frostglow") {
                        return true;
                    }
                    if lower.contains("dark") {
                        return false;
                    }
                }
            }
        }
    }

    // 5. Check Alacritty config header
    if let Ok(home) = std::env::var("HOME") {
        let alacritty_config = format!("{}/.config/alacritty/alacritty.toml", home);
        if let Ok(content) = fs::read_to_string(&alacritty_config) {
            let header: String = content
                .lines()
                .take(10)
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if header.contains("frostglow") || header.contains("light") {
                return true;
            }
            if header.contains("deep cracked ice") || header.contains("dark") {
                return false;
            }
        }
    }

    // 6. Check terminal-specific theme env vars
    for var in ["ALACRITTY_THEME", "GHOSTTY_THEME"] {
        if let Ok(theme) = std::env::var(var) {
            return theme.to_lowercase().contains("light");
        }
    }

    // Default: dark
    false
}
