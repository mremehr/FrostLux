use crate::tradfri::{COLOR_TEMP_COLD, COLOR_TEMP_NEUTRAL};

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
