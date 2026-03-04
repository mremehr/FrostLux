pub mod config;
pub mod scene;

pub use config::*;
pub use scene::*;

use anyhow::Result;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::coap::{self, SharedTradfriClient};
use crate::tradfri::{self, Light, COLOR_TEMP_LABELS, COLOR_TEMPS};

/// Ensures the periodic refresh triggers immediately at startup rather than
/// waiting a full `refresh_interval` before the first background fetch.
const INITIAL_REFRESH_OFFSET: Duration = Duration::from_secs(999);

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
            // Subtract INITIAL_REFRESH_OFFSET so the first background refresh
            // fires immediately once a client is connected.
            last_refresh: Instant::now() - INITIAL_REFRESH_OFFSET,
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
                if let Err(e) = tradfri::set_power(&client, light.id, new_state) {
                    tracing::warn!("set_power failed for '{}': {}", light.name, e);
                }
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
                if let Err(e) = tradfri::set_brightness(&client, &light, new_brightness) {
                    tracing::warn!("set_brightness failed for '{}': {}", light.name, e);
                }
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
                if let Err(e) = tradfri::set_color_temp(&client, &light, &hex) {
                    tracing::warn!("set_color_temp failed for '{}': {}", light.name, e);
                }
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
                if let Err(e) = client.apply_scene_to_light(id, on, brightness, &color) {
                    tracing::warn!("apply_scene failed for light id {}: {}", id, e);
                }
            }
        });
        Ok(())
    }

    /// Run a scene in headless mode (no TUI) — for CLI usage.
    pub fn run_scene_headless(config: &Config, scene: Scene) -> Result<()> {
        use anyhow::Context;
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
