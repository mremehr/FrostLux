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
            background: Color::Reset,
            foreground: Color::Rgb(245, 250, 255),

            ice_blue: Color::Rgb(100, 200, 255),
            cold_green: Color::Rgb(80, 250, 150),
            bright_red: Color::Rgb(255, 95, 135),
            warm_yellow: Color::Rgb(255, 215, 95),
            crystal_cyan: Color::Rgb(95, 215, 255),

            border: Color::Rgb(60, 90, 120),
            dimmed: Color::Rgb(80, 100, 120),
        }
    }

    pub fn frostglow_light() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Rgb(15, 25, 35),

            ice_blue: Color::Rgb(30, 120, 200),
            cold_green: Color::Rgb(15, 140, 80),
            bright_red: Color::Rgb(220, 50, 70),
            warm_yellow: Color::Rgb(200, 130, 30),
            crystal_cyan: Color::Rgb(20, 150, 200),

            border: Color::Rgb(160, 190, 210),
            dimmed: Color::Rgb(140, 155, 170),
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

    // 2. Check COLORFGBG (xterm, rxvt, Ghostty, etc.)
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

    // 3. Check Ghostty config
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

    // 4. Check Alacritty marker file
    if let Ok(theme_marker) = fs::read_to_string(
        std::env::var("HOME")
            .map(|h| format!("{}/.config/alacritty/.current-theme", h))
            .unwrap_or_default(),
    ) {
        let theme = theme_marker.trim().to_lowercase();
        if theme.contains("light") || theme.contains("frostglow") {
            return true;
        }
        if theme.contains("dark") || theme.contains("cracked") || theme.contains("ice") {
            return false;
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
