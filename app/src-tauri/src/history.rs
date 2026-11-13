//! Finished fights saved as JSON files, one per fight, in a folder the user can choose.

use crate::meter::EncounterView;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct FightFile {
    version: u32,
    fight: EncounterView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFight {
    /// File name inside the history folder; the id used to delete it.
    pub file: String,
    pub fight: EncounterView,
}

/// "2026-11-11_21-04-33_grimmag.json" (local time of the fight's start).
fn file_name(fight: &EncounterView) -> String {
    let target = fight
        .killed
        .as_deref()
        .or_else(|| fight.targets.iter().map(|t| t.name.as_str()).find(|n| *n != "unknown"))
        .unwrap_or("fight");
    let slug: String = target
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "fight".to_string() } else { slug.chars().take(40).collect() };
    format!("{}_{slug}.json", local_stamp(fight.start))
}

/// Local date and time as YYYY-MM-DD_HH-MM-SS, without pulling in a date crate.
fn local_stamp(epoch: f64) -> String {
    let secs = epoch as i64 + local_offset_secs();
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

#[cfg(windows)]
fn local_offset_secs() -> i64 {
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        rest: [u8; 168],
    }
    unsafe extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
    }
    let mut tz = TimeZoneInformation { bias: 0, rest: [0; 168] };
    // 0 = unknown, 1 = standard time, 2 = daylight time.
    let mode = unsafe { GetTimeZoneInformation(&mut tz) };
    // Layout: Bias, StandardName[32], StandardDate, StandardBias, DaylightName[32], DaylightDate, DaylightBias.
    let i32_at = |off: usize| i32::from_le_bytes(tz.rest[off..off + 4].try_into().unwrap());
    let standard_bias = i32_at(64 + 16);
    let daylight_bias = i32_at(64 + 16 + 4 + 64 + 16);
    let bias = tz.bias + if mode == 2 { daylight_bias } else { standard_bias };
    -(bias as i64) * 60
}

#[cfg(not(windows))]
fn local_offset_secs() -> i64 {
    0
}

pub fn save(dir: &Path, fight: &EncounterView) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
    let mut path = dir.join(file_name(fight));
    // Two fights starting in the same second: add a counter.
    let mut n = 2;
    while path.exists() {
        path = dir.join(file_name(fight).replace(".json", &format!("_{n}.json")));
        n += 1;
    }
    let json = serde_json::to_string_pretty(&FightFile { version: VERSION, fight: fight.clone() })
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Can't write {}: {e}", path.display()))?;
    Ok(path)
}

/// Every readable fight in the folder, newest first.
pub fn list(dir: &Path) -> Vec<SavedFight> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<SavedFight> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            let f: FightFile = serde_json::from_str(&text).ok()?;
            Some(SavedFight { file: e.file_name().to_string_lossy().into_owned(), fight: f.fight })
        })
        .collect();
    out.sort_by(|a, b| b.fight.start.total_cmp(&a.fight.start));
    out
}

pub fn delete(dir: &Path, file: &str) -> Result<(), String> {
    // Only plain file names from `list`, never a path.
    if file.contains(['/', '\\', ':']) || file.contains("..") || !file.ends_with(".json") {
        return Err("Not a saved fight.".into());
    }
    std::fs::remove_file(dir.join(file)).map_err(|e| e.to_string())
}
