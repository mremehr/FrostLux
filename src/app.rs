use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crate::coap::{self, SharedTradfriClient};
use crate::tradfri::{self, Light, COLOR_TEMP_COLD, COLOR_TEMP_NEUTRAL, COLOR_TEMPS, COLOR_TEMP_LABELS};

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

// ── Scenes ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scene {
    AllOn,
    AllOff,
    Movie,
    Bright,
    Cozy,
    Night,
    Evening,
    Reading,
    GoodMorning,
}

impl Scene {
    pub fn config_key(&self) -> &'static str {
        match self {
            Scene::AllOn => "on",
            Scene::AllOff => "off",
            Scene::Movie => "movie",
            Scene::Bright => "bright",
            Scene::Cozy => "cozy",
            Scene::Night => "night",
            Scene::Evening => "evening",
            Scene::Reading => "reading",
            Scene::GoodMorning => "morning",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Scene::AllOn => "All On",
            Scene::AllOff => "All Off",
            Scene::Movie => "Movie",
            Scene::Bright => "Bright",
            Scene::Cozy => "Cozy",
            Scene::Night => "Night",
            Scene::Evening => "Evening",
            Scene::Reading => "Reading",
            Scene::GoodMorning => "Good Morning",
        }
    }

    /// Returns (on, brightness 0-254, color_hex).
    pub fn settings(&self) -> (bool, u8, &str) {
        match self {
            Scene::AllOn       => (true,  254, COLOR_TEMP_COLD),
            Scene::AllOff      => (false, 0,   COLOR_TEMP_COLD),
            Scene::Movie       => (true,  30,  COLOR_TEMP_NEUTRAL),
            Scene::Bright      => (true,  254, COLOR_TEMP_COLD),
            Scene::Cozy        => (true,  127, COLOR_TEMP_NEUTRAL),
            Scene::Night       => (true,  15,  COLOR_TEMP_NEUTRAL),
            Scene::Evening     => (true,  150, COLOR_TEMP_NEUTRAL),
            Scene::Reading     => (true,  200, COLOR_TEMP_COLD),
            Scene::GoodMorning => (true,  180, COLOR_TEMP_COLD),
        }
    }

    /// Parse scene name from string (for CLI).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "allon" | "all-on" => Some(Scene::AllOn),
            "off" | "alloff" | "all-off" => Some(Scene::AllOff),
            "movie" | "film" => Some(Scene::Movie),
            "bright" | "ljus" => Some(Scene::Bright),
            "cozy" | "mysig" => Some(Scene::Cozy),
            "night" | "natt" => Some(Scene::Night),
            "evening" | "kväll" | "kvall" => Some(Scene::Evening),
            "reading" | "läsning" | "lasning" => Some(Scene::Reading),
            "morning" | "good-morning" | "morgon" => Some(Scene::GoodMorning),
            _ => None,
        }
    }

    pub fn all() -> &'static [Scene] {
        &[
            Scene::AllOn, Scene::AllOff, Scene::Movie, Scene::Bright,
            Scene::Cozy, Scene::Night, Scene::Evening, Scene::Reading,
            Scene::GoodMorning,
        ]
    }

}

// ── Startup result ──────────────────────────────────────

enum StartupResult {
    Connected { client: SharedTradfriClient, lights: Vec<Light> },
    Failed(String),
}

// ── App State ───────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub client: Option<SharedTradfriClient>,
    pub lights: Vec<Light>,
    pub selected: usize,
    pub should_quit: bool,
    pub status_msg: Option<(String, Instant)>,
    pub last_refresh: Instant,
    pub show_help: bool,
    pub is_connecting: bool,
    refresh_tx: mpsc::Sender<Vec<Light>>,
    refresh_rx: mpsc::Receiver<Vec<Light>>,
    startup_rx: Option<mpsc::Receiver<StartupResult>>,
}

impl App {
    /// Create app and immediately kick off a background connection + parallel
    /// light fetch. The TUI is shown instantly; lights appear when ready.
    pub fn new(config: Config) -> Self {
        let (refresh_tx, refresh_rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::channel::<StartupResult>();

        let host = config.gateway.host.clone();
        let identity = config.gateway.identity.clone();
        let psk = config.gateway.psk.clone();

        std::thread::spawn(move || {
            // One operation: connect, fetch lights in parallel, reuse connection as client.
            match coap::connect_and_fetch_lights(&host, &identity, &psk) {
                Ok((infos, client)) => {
                    let mut lights: Vec<Light> = infos.into_iter().map(Light::from).collect();
                    lights.sort_by(|a, b| a.name.cmp(&b.name));
                    let _ = startup_tx.send(StartupResult::Connected { client, lights });
                }
                Err(e) => {
                    let _ = startup_tx.send(StartupResult::Failed(e.to_string()));
                }
            }
        });

        Self {
            config,
            client: None,
            lights: Vec::new(),
            selected: 0,
            should_quit: false,
            status_msg: None,
            last_refresh: Instant::now() - std::time::Duration::from_secs(999),
            show_help: false,
            is_connecting: true,
            refresh_tx,
            refresh_rx,
            startup_rx: Some(startup_rx),
        }
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_msg = Some((msg.to_string(), Instant::now()));
    }

    pub fn current_status(&self) -> Option<&str> {
        if self.is_connecting {
            return Some("Ansluter till gateway...");
        }
        if let Some((msg, time)) = &self.status_msg {
            if time.elapsed().as_secs() < 3 {
                return Some(msg);
            }
        }
        None
    }

    pub fn select_next(&mut self) {
        if !self.lights.is_empty() {
            self.selected = (self.selected + 1).min(self.lights.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Start a non-blocking background refresh (only when connected).
    pub fn start_background_refresh(&mut self) {
        let Some(client) = self.client.clone() else { return };
        self.last_refresh = Instant::now();
        let tx = self.refresh_tx.clone();
        std::thread::spawn(move || {
            if let Ok(lights) = tradfri::fetch_lights(&client) {
                let _ = tx.send(lights);
            }
        });
    }

    /// Poll for startup result and periodic refresh results.
    pub fn poll_refresh(&mut self) {
        // Handle initial connection result.
        if let Some(rx) = &self.startup_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    StartupResult::Connected { client, lights } => {
                        self.client = Some(client);
                        self.lights = lights;
                        self.is_connecting = false;
                        self.last_refresh = Instant::now();
                        if self.selected >= self.lights.len() {
                            self.selected = self.lights.len().saturating_sub(1);
                        }
                    }
                    StartupResult::Failed(err) => {
                        self.is_connecting = false;
                        self.set_status(&format!("Anslutning misslyckades: {}", err));
                    }
                }
                self.startup_rx = None;
            }
        }

        // Handle periodic background refresh.
        if let Ok(lights) = self.refresh_rx.try_recv() {
            self.lights = lights;
            if self.selected >= self.lights.len() {
                self.selected = self.lights.len().saturating_sub(1);
            }
        }
    }

    pub fn toggle_selected(&mut self) -> Result<()> {
        let Some(client) = self.client.clone() else {
            self.set_status("Väntar på anslutning...");
            return Ok(());
        };
        if let Some(light) = self.lights.get(self.selected).cloned() {
            let new_state = !light.on;
            if let Some(l) = self.lights.get_mut(self.selected) {
                l.on = new_state;
            }
            self.set_status(&format!("{}: {}", light.name, if new_state { "ON" } else { "OFF" }));
            std::thread::spawn(move || {
                let _ = tradfri::set_power(&client, light.id, new_state);
            });
        }
        Ok(())
    }

    pub fn dim_selected(&mut self, delta: i16) -> Result<()> {
        let Some(client) = self.client.clone() else {
            self.set_status("Väntar på anslutning...");
            return Ok(());
        };
        if let Some(light) = self.lights.get(self.selected).cloned() {
            let new_brightness = (light.brightness as i16 + delta).clamp(0, 254) as u8;
            if let Some(l) = self.lights.get_mut(self.selected) {
                l.brightness = new_brightness;
                l.on = new_brightness > 0;
            }
            let pct = ((new_brightness as f32 / 254.0) * 100.0).round() as u8;
            self.set_status(&format!("{}: {}%", light.name, pct));
            std::thread::spawn(move || {
                let _ = tradfri::set_brightness(&client, &light, new_brightness);
            });
        }
        Ok(())
    }

    pub fn cycle_color_temp(&mut self, warmer: bool) -> Result<()> {
        let Some(client) = self.client.clone() else {
            self.set_status("Väntar på anslutning...");
            return Ok(());
        };
        if let Some(light) = self.lights.get(self.selected).cloned() {
            let temps = COLOR_TEMPS;
            let labels = COLOR_TEMP_LABELS;
            let current_idx = temps.iter().position(|&h| Some(h) == light.color_hex.as_deref());
            let new_idx = match (current_idx, warmer) {
                (Some(i), true) => (i + 1).min(temps.len() - 1),
                (Some(i), false) => i.saturating_sub(1),
                (None, true) => temps.len() - 1,
                (None, false) => 0,
            };
            if let Some(l) = self.lights.get_mut(self.selected) {
                l.color_hex = Some(temps[new_idx].to_string());
            }
            self.set_status(&format!("{}: {}", light.name, labels[new_idx]));
            let hex = temps[new_idx].to_string();
            std::thread::spawn(move || {
                let _ = tradfri::set_color_temp(&client, &light, &hex);
            });
        }
        Ok(())
    }

    /// Apply a scene to all non-excluded lights.
    pub fn apply_scene(&mut self, scene: Scene) -> Result<()> {
        let Some(client) = self.client.clone() else {
            self.set_status("Väntar på anslutning...");
            return Ok(());
        };
        let (on, brightness, color) = scene.settings();
        let scenes_cfg = &self.config.scenes;
        let targets: Vec<u64> = self.lights.iter()
            .filter(|l| !scenes_cfg.is_excluded_for_scene(scene, &l.name))
            .map(|l| l.id)
            .collect();
        for light in &mut self.lights {
            if !scenes_cfg.is_excluded_for_scene(scene, &light.name) {
                light.on = on;
                if on {
                    light.brightness = brightness;
                    light.color_hex = Some(color.to_string());
                }
            }
        }
        self.set_status(&format!("Scene: {}", scene.name()));
        let color = color.to_string();
        std::thread::spawn(move || {
            for id in targets {
                let _ = client.apply_scene_to_light(id, on, brightness, &color);
            }
        });
        Ok(())
    }

    /// Run a scene in headless mode (no TUI) — for CLI usage.
    pub fn run_scene_headless(config: &Config, scene: Scene) -> Result<()> {
        let client = SharedTradfriClient::new(
            &config.gateway.host,
            &config.gateway.identity,
            &config.gateway.psk,
        ).context("Failed to connect to Trådfri gateway")?;

        let (on, brightness, color) = scene.settings();
        let lights = client.list_lights()?;
        for light in &lights {
            if !config.scenes.is_excluded_for_scene(scene, &light.name) {
                client.apply_scene_to_light(light.id, on, brightness, color)?;
            }
        }
        println!("FrostLux: {} applied", scene.name());
        Ok(())
    }

    pub fn lights_on(&self) -> usize {
        self.lights.iter().filter(|l| l.on).count()
    }

    pub fn lights_off(&self) -> usize {
        self.lights.iter().filter(|l| !l.on).count()
    }
}
