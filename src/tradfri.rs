use crate::coap::{LightInfo, SharedTradfriClient};
use anyhow::Result;

/// Light representation for the TUI
#[derive(Debug, Clone)]
pub struct Light {
    pub id: u64,
    pub name: String,
    pub on: bool,
    /// 0-254
    pub brightness: u8,
    /// Color hex string (e.g. "f1e0b5" for warm)
    pub color_hex: Option<String>,
    pub reachable: bool,
}

impl From<LightInfo> for Light {
    fn from(info: LightInfo) -> Self {
        Self {
            id: info.id,
            name: info.name,
            on: info.on,
            brightness: info.brightness,
            color_hex: info.color_hex,
            reachable: info.reachable,
        }
    }
}

impl Light {
    /// Brightness as percentage (0-100).
    pub fn brightness_percent(&self) -> u8 {
        ((self.brightness as f32 / 254.0) * 100.0).round() as u8
    }

    /// Color temperature label based on hex.
    pub fn color_temp_label(&self) -> &str {
        match self.color_hex.as_deref() {
            Some("f5faf6") => "cold",
            Some("f1e0b5") => "neutral",
            Some("efd275") => "warm",
            Some(h) if h.starts_with("f5") => "cold",
            Some(h) if h.starts_with("efd") => "warm",
            Some(_) => "neutral",
            None => "",
        }
    }
}

/// Fetch all lights from the gateway
pub fn fetch_lights(client: &SharedTradfriClient) -> Result<Vec<Light>> {
    let infos = client.list_lights()?;
    let mut lights: Vec<Light> = infos.into_iter().map(Light::from).collect();
    lights.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(lights)
}

/// Set a light on/off.
pub fn set_power(client: &SharedTradfriClient, light_id: u64, on: bool) -> Result<()> {
    client.set_power(light_id, on)
}

/// Set brightness (0-254). Also turns the light on if brightness > 0.
pub fn set_brightness(client: &SharedTradfriClient, light: &Light, brightness: u8) -> Result<()> {
    client.set_brightness(light.id, brightness)
}

/// Set color temperature by hex value.
pub fn set_color_temp(client: &SharedTradfriClient, light: &Light, hex: &str) -> Result<()> {
    client.set_color(light.id, hex)
}
