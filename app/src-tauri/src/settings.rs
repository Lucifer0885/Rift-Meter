use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlaySettings {
    pub visible: bool,
    /// Mouse clicks pass through to the game (overlay can't be dragged while on).
    pub click_through: bool,
    /// Overlay can't be dragged or reset by clicking it (move it from the Layout page instead).
    pub locked: bool,
    /// Background opacity, 0..1.
    pub opacity: f64,
    pub scale: f64,
    /// Width of the overlay in pixels before scaling (Size multiplies it).
    pub width: f64,
    /// Party row colors: "class" (by class) or "random" (a new color per player each fight).
    pub row_colors: String,
    /// Class name -> "#rrggbb", overriding the default class colors. Empty = defaults.
    pub class_colors: BTreeMap<String, String>,
    pub show_dps: bool,
    pub show_total: bool,
    pub show_crit: bool,
    pub show_max_hit: bool,
    pub show_timer: bool,
    /// Share of the party's damage (party view).
    pub show_share: bool,
    /// One row per party member instead of only your own numbers.
    pub party_mode: bool,
    /// Per-source rows (direct hits, each summon type) in the one-character view.
    pub show_skills: bool,
    /// How many times each skill was used (your own character).
    pub show_casts: bool,
    /// Damage split by element (Fire, Ice, Lightning, ...) per character and skill.
    pub show_elements: bool,
    pub max_rows: usize,
    /// HP bar for the current target. Off by default.
    pub show_target_hp: bool,
    /// Rank categories that get an HP bar.
    pub hp_ranks: Vec<String>,
    pub show_hp_name: bool,
    /// "14,546,156 / 38,564,199"
    pub show_hp_numbers: bool,
    pub show_hp_pct: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            visible: true,
            click_through: false,
            locked: false,
            opacity: 0.85,
            scale: 1.0,
            width: 320.0,
            row_colors: "class".into(),
            class_colors: BTreeMap::new(),
            show_dps: true,
            show_total: true,
            show_crit: true,
            show_max_hit: true,
            show_timer: true,
            show_share: true,
            party_mode: false,
            show_skills: true,
            show_casts: true,
            show_elements: false,
            max_rows: 6,
            show_target_hp: false,
            hp_ranks: ["guardian", "champion", "leader", "boss", "worldboss"].map(String::from).to_vec(),
            show_hp_name: true,
            show_hp_numbers: true,
            show_hp_pct: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Npcap device name; None = pick the adapter that reaches the internet.
    pub interface: Option<String>,
    pub auto_start: bool,
    /// Seconds without hits before a fight is over.
    pub idle_timeout: f64,
    /// Stop the fight timer when a boss dies.
    pub stop_on_boss_kill: bool,
    /// Rank categories to count (monster, guardian, champion, leader, boss, worldboss).
    /// Empty = count every target.
    pub ranks: Vec<String>,
    /// When a rank filter is set, also count targets whose rank is unknown.
    pub include_unknown: bool,
    /// Write every finished fight to a JSON file.
    pub save_fights: bool,
    /// Folder for saved fights; None = the app's data folder.
    pub history_dir: Option<String>,
    /// Fights shorter than this aren't saved (trash mobs while farming).
    pub min_saved_fight: f64,
    /// Global key combination that shows/hides the overlay ("Ctrl+Shift+O"); None = off.
    pub overlay_hotkey: Option<String>,
    pub overlay: OverlaySettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interface: None,
            auto_start: true,
            idle_timeout: 30.0,
            stop_on_boss_kill: true,
            ranks: Vec::new(),
            include_unknown: false,
            save_fights: true,
            history_dir: None,
            min_saved_fight: 10.0,
            overlay_hotkey: Some("Ctrl+Shift+O".into()),
            overlay: OverlaySettings::default(),
        }
    }
}
