use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

use super::scene::Scene;

const CONFIG_FILENAME: &str = "config.toml";

// ── Config ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub scenes: ScenesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub psk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_refresh")]
    pub refresh_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenesConfig {
    /// Light names to exclude from all scene commands.
    /// Example: exclude = ["Sovrummet", "Barnrummet"]
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Scene-specific exclusions by scene key.
    /// Example: exclude_by_scene = { movie = ["TV Lamp"], night = ["Kitchen"] }
    #[serde(default)]
    pub exclude_by_scene: HashMap<String, Vec<String>>,
}

impl ScenesConfig {
    pub fn is_excluded_for_scene(&self, scene: Scene, light_name: &str) -> bool {
        if self
            .exclude
            .iter()
            .any(|e| e.eq_ignore_ascii_case(light_name))
        {
            return true;
        }

        let scene_key = scene.config_key();
        self.exclude_by_scene.iter().any(|(key, names)| {
            key.eq_ignore_ascii_case(scene_key)
                && names.iter().any(|e| e.eq_ignore_ascii_case(light_name))
        })
    }
}

fn default_host() -> String { "192.168.0.131".to_string() }
fn default_theme() -> String { "auto".to_string() }
fn default_refresh() -> u64 { 5 }

impl Default for Config {
    fn default() -> Self {
        Self {
            gateway: GatewayConfig {
                host: default_host(),
                identity: String::new(),
                psk: String::new(),
            },
            ui: UiConfig {
                theme: default_theme(),
                refresh_interval: default_refresh(),
            },
            scenes: ScenesConfig::default(),
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            identity: String::new(),
            psk: String::new(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            refresh_interval: default_refresh(),
        }
    }
}

// ── Config loading ──────────────────────────────────────

fn config_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("frostlux");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".config").join("frostlux");
    }
    PathBuf::from("config")
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("frostlux").join(CONFIG_FILENAME));
    }
    if let Ok(home) = env::var("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join(".config")
                .join("frostlux")
                .join(CONFIG_FILENAME),
        );
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("config").join("default.toml"));
        }
    }
    paths.push(PathBuf::from("config").join("default.toml"));

    paths
}

pub fn load_config() -> Result<Config> {
    for path in config_paths() {
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;

            if !config.gateway.host.is_empty() {
                config
                    .gateway
                    .host
                    .parse::<IpAddr>()
                    .with_context(|| format!("Invalid gateway IP: '{}'", config.gateway.host))?;
            }

            return Ok(config);
        }
    }

    // Auto-generate default config
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(CONFIG_FILENAME);
    let default = Config::default();
    let content = format!(
        "# FrostLux Configuration\n\
         # Edit this file to customize FrostLux behavior.\n\n\
         [gateway]\n\
         host = \"{}\"\n\
         identity = \"\"  # From gateway pairing\n\
         psk = \"\"        # Pre-shared key\n\n\
         [ui]\n\
         theme = \"auto\"  # auto, light, dark\n\
         refresh_interval = 5\n\n\
         [scenes]\n\
         # Lights to exclude from all scene commands:\n\
         # exclude = [\"Sovrummet\", \"Barnrummet\"]\n\
         exclude = []\n\
         # Exclude only for specific scenes (keys: on, off, movie, bright,\n\
         # cozy, night, evening, reading, morning)\n\
         # exclude_by_scene = {{ movie = [\"TV\"], night = [\"Kitchen\"] }}\n\
         exclude_by_scene = {{}}\n",
        default.gateway.host
    );
    fs::write(&path, &content)?;
    eprintln!("Generated default config at: {}", path.display());
    eprintln!("Edit it with your gateway credentials before running FrostLux.");
    Ok(default)
}
