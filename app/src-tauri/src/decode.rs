//! Find damage hits, entity spawns and skill casts in DSO game messages.
//! The comments next to each layout say which capture it was worked out from.
//!
//! Most fields are not byte-aligned, so every search runs on the message shifted by 0..7 bits.

use regex::bytes::Regex;
use serde::Serialize;
use std::sync::LazyLock;

pub const SERVER_BUNDLE: u8 = 0x85;
pub const CLIENT_BUNDLE: u8 = 0x8b;
/// Skill use: sub-messages 0x46..=0x4a (by targeting type), `00 00 00 [u16 skill id]` after the type.
// 0x4b: ground-targeted casts with a longer payload (Bird of Prey).
const CAST_TYPES: std::ops::RangeInclusive<u8> = 0x46..=0x4b;
const MAX_HIT_MESSAGE: usize = 100_000; // skip zone-load snapshots when looking for hits
const MAX_SPAWN_MESSAGE: usize = 5_000_000; // ...but read them for spawns

// Hit: [kind u8] [u64 target hp after hit] [u64 target max hp] [4 zero] [u32 ?]
//      [u32 LE damage] 03 00 00 00 [u32 attacker] [u32 owner] ... [u32 target] ff
// kind is a set of flags: 0x01 = Rift Echo, 0x02 = crit, bits 3-5 = damage type (see
// kind_element). None of it is used to filter.
// Mob attacks are dropped by the meter, which knows which entities are mobs.
const KIND_OFFSET: usize = 25;
const KIND_CRIT: u8 = 0x02;
/// Bit 0: Rift Echo from the Rift Force vessel. Seen only with Rift Force equipped
/// (captures/2026-11-12_rift_force.pcapng: kinds 01/03 in the kill with it, none in the kill
/// without); each echo is a fixed multiple of the hit before it and is real HP loss.
const KIND_ECHO: u8 = 0x01;

/// Damage type of a hit from its kind byte, bits 3-5. The values follow the client's damage-type
/// table (Physical, Fire, Ice, Lightning, DarkMagic, Poison at 0x1411a1250 in dro_client64.exe);
/// confirmed in captures: 0x10/0x12 Ice Missile, 0x08-0x0b Ember Knight Kain, 0x28/0x2a Hunting
/// Arrow with poison mastery and Medusa's gorgons. DarkMagic is shown in game as Andermagic.
/// None for 0 (Physical): training dummies report 0 for every skill, so the meter decides.
pub fn kind_element(kind: u8) -> Option<&'static str> {
    Some(match (kind >> 3) & 7 {
        1 => "Fire",
        2 => "Ice",
        3 => "Lightning",
        4 => "Andermagic",
        5 => "Poison",
        _ => return None,
    })
}
/// Bytes `00 ff 4e 00`: the owner->summon link message (sub-message 0x4e) read as a hit.
const SUMMON_LINK_FALSE_HIT: u32 = 5_177_088;
/// The 4 bytes before the last u32 are zero in every real hit; the "<owner> <summon>" message
/// (sub-message 0x4e) looks like a hit but has data there.
const ZERO_PAD: (usize, usize) = (8, 4);
/// How far after the hit to look for the target id.
const TARGET_SEARCH: usize = 400;

static HIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)(.{4})\x03\x00\x00\x00(..\x01\x00)(..\x01\x00)").unwrap());
/// Effect damage (burns, frost explosions, ...): same fields as a hit, but no attacker:
/// [kind] [u64 hp] [u64 max] 00000000 [u32 ?] [u32 damage] ff ff ff ff ... "DamageDone<Element>" [u32 target] ff
static EFFECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s-u)(.{8})(.{8})\x00\x00\x00\x00.{4}(.{4})\xff\xff\xff\xff(..\x01\x00)?")
        .unwrap()
});
static EFFECT_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s-u)[\x0a-\x18]\x00Damage(Done|Received)([A-Za-z]*)(..\x01\x00)\xff").unwrap()
});
/// How far after an effect record its tag string can be.
const EFFECT_TAG_SEARCH_BITS: usize = 96 * 8;
static TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s-u)(..\x01\x00)\xff").unwrap());
// Spawn: 2a 00 [u16 len] [template] ... [u16 len] "Rank_xxx" [u16 len] [extra, e.g. "boss_basic"]
//        [u32 id] ff
static SPAWN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)\x2a\x00(..)[A-Za-z0-9_]{4}").unwrap());
// Player info (sub-message 0x97), sent on zone entry:
//   [u32 player id] [u16 len] [name, UTF-8] [u16 len] [zone or empty] [u8 class] [u16 level] ...
// Names can be non-ASCII (e.g. Cyrillic letters), so only the start is matched here.
static NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s-u)(..\x01\x00)([\x03-\x30])\x00[A-Za-z0-9\xc0-\xff]").unwrap()
});
/// Class numbers in the player info, in the client's order (ui/emblem_class_* at 0x140f49610):
/// confirmed 1 = Spellweaver (mage), 2 = Ranger.
const CLASSES: [&str; 4] = ["Dragonknight", "Spellweaver", "Ranger", "Steam Mechanicus"];
const PLAYER_INFO: u8 = 0x97;
/// The same record for some characters, without the zone name (captures/2026-11-13_character_switch).
const PLAYER_INFO_SHORT: u8 = 0xa7;

/// Character names: letters, digits, `_`, `-` and spaces inside ("Saint LuсiфеЯ").
fn valid_name(name: &str) -> bool {
    name.chars().count() >= 3
        && !name.starts_with(' ')
        && !name.ends_with(' ')
        && !name.contains("  ")
        && name.chars().all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == ' ')
}
static RANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s-u)(..)Rank_").unwrap());

fn is_ident(b: &[u8]) -> bool {
    b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// Parse `[u16 len] Rank_xxx [u16 len] [extra] [u32 id] ff` at `pos` (the rank's length field).
fn parse_rank(x: &[u8], pos: usize) -> Option<(String, u32)> {
    let rlen = u16::from_le_bytes([*x.get(pos)?, *x.get(pos + 1)?]) as usize;
    if !(6..=48).contains(&rlen) {
        return None;
    }
    let rank = x.get(pos + 2..pos + 2 + rlen)?;
    if !is_ident(rank) {
        return None;
    }
    let mut p = pos + 2 + rlen;
    let elen = u16::from_le_bytes([*x.get(p)?, *x.get(p + 1)?]) as usize;
    if elen > 48 || !is_ident(x.get(p + 2..p + 2 + elen)?) {
        return None;
    }
    p += 2 + elen;
    let id = x.get(p..p + 5)?;
    if id[2] != 0x01 || id[3] != 0x00 || id[4] != 0xff {
        return None;
    }
    Some((String::from_utf8_lossy(rank).into_owned(), le32(id)))
}
const RANK_SEARCH_BITS: usize = 600 * 8;

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub damage: u32,
    pub attacker: u32,
    pub owner: u32,
    /// 0 = not found in the record (killing blows from summons); the meter then matches the
    /// target by max HP.
    pub target: u32,
    pub crit: bool,
    /// Target HP left after this hit (0 = killed).
    pub target_hp: u64,
    pub target_max_hp: u64,
    /// (bit shift, byte offset of the damage) inside the message, for debugging tools.
    #[serde(skip)]
    pub at: (u32, usize),
    /// Effect damage (burn ticks, frost explosions...): no attacker in the record; the server
    /// only sends these for your own damage ("DamageDone").
    pub effect: bool,
    /// Fire / Ice / Lightning / Poison / Andermagic, when the record names it.
    pub element: Option<String>,
    /// Rift Echo (Rift Force arcane vessel): an extra hit right after the one that triggered it,
    /// flagged by bit 0 of the kind byte.
    pub echo: bool,
}

fn le64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap())
}

#[derive(Clone, Debug)]
pub struct Spawn {
    pub id: u32,
    pub template: String,
    pub rank: String,
}

/// The message shifted left by `k` bits, same length.
pub fn shifted(data: &[u8], k: u32) -> Vec<u8> {
    if k == 0 {
        return data.to_vec();
    }
    let n = data.len();
    let mut out = vec![0u8; n];
    for i in 0..n {
        let next = if i + 1 < n { data[i + 1] } else { 0 };
        out[i] = (data[i] << k) | (next >> (8 - k));
    }
    out
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub fn find_hits(msg: &[u8]) -> Vec<Hit> {
    let mut out = Vec::new();
    if msg.len() < 40 || msg[0] != SERVER_BUNDLE || msg.len() > MAX_HIT_MESSAGE {
        return out;
    }
    for k in 0..8 {
        let x = shifted(msg, k);
        for c in HIT.captures_iter(&x) {
            let start = c.get(0).unwrap().start();
            if start < KIND_OFFSET {
                continue;
            }
            let kind = x[start - KIND_OFFSET];
            if kind & 0xc0 != 0 {
                continue;
            }
            if x[start - ZERO_PAD.0..start - ZERO_PAD.1]
                .iter()
                .any(|&b| b != 0)
            {
                continue;
            }
            let hp = le64(&x[start - 24..]);
            let max_hp = le64(&x[start - 16..]);
            if hp > max_hp || max_hp == 0 {
                continue;
            }
            let damage = le32(&c[1]);
            if damage <= 1 || damage == SUMMON_LINK_FALSE_HIT {
                continue;
            }
            let (attacker, owner) = (le32(&c[2]), le32(&c[3]));
            // First "<id> ff" after the hit that isn't the attacker itself (a killing blow can
            // carry the attacker's id first).
            let end = c.get(0).unwrap().end();
            let limit = (end + TARGET_SEARCH).min(x.len());
            let target = TARGET
                .captures_iter(&x[end..limit])
                .map(|t| le32(&t[1]))
                .find(|id| *id != attacker && *id != owner);
            let target = match target {
                Some(t) => t,
                None if max_hp > 0 => 0,
                None => continue,
            };
            out.push(Hit {
                damage,
                attacker,
                owner,
                target,
                crit: kind & KIND_CRIT != 0,
                target_hp: hp,
                target_max_hp: max_hp,
                at: (k, start),
                effect: false,
                element: kind_element(kind).map(String::from),
                echo: kind & KIND_ECHO != 0,
            });
        }
    }
    out
}

/// Effect damage records: burns, frost explosions and other damage without an attacker.
/// Each is paired with the "DamageDone<Element>" tag after it, which also names the target.
pub fn find_effect_hits(msg: &[u8]) -> Vec<Hit> {
    let mut out = Vec::new();
    if msg.len() < 60 || msg[0] != SERVER_BUNDLE || msg.len() > MAX_HIT_MESSAGE {
        return out;
    }
    // (bit position, received?, element, target)
    let mut tags: Vec<(usize, bool, String, u32)> = Vec::new();
    // (bit end, k, crit, hp, max, dmg, kind, owner if the record ends with one)
    let mut recs: Vec<(usize, u32, bool, u64, u64, u32, u8, Option<u32>)> = Vec::new();
    for k in 0..8u32 {
        let x = shifted(msg, k);
        for c in EFFECT_TAG.captures_iter(&x) {
            let bit = c.get(0).unwrap().start() * 8 + k as usize;
            tags.push((
                bit,
                &c[1] == b"Received",
                String::from_utf8_lossy(&c[2]).into_owned(),
                le32(&c[3]),
            ));
        }
        for c in EFFECT.captures_iter(&x) {
            let start = c.get(0).unwrap().start();
            if start < 1 {
                continue;
            }
            let kind = x[start - 1];
            let hp = le64(&c[1]);
            let max = le64(&c[2]);
            let dmg = le32(&c[3]);
            if kind & 0xc0 != 0 || max == 0 || hp > max || max > 1_000_000_000_000 || dmg == 0 {
                continue;
            }
            let owner = c.get(4).map(|m| le32(m.as_bytes()));
            // End before the owner id, so tags are measured from the damage fields as before.
            let end = c.get(3).unwrap().end() + 4;
            recs.push((
                end * 8 + k as usize,
                k,
                kind & KIND_CRIT != 0,
                hp,
                max,
                dmg,
                kind,
                owner,
            ));
        }
    }
    tags.sort_by_key(|t| t.0);
    recs.sort_by_key(|r| r.0);
    recs.dedup_by_key(|r| (r.3, r.4, r.5));
    let mut used = vec![false; recs.len()];
    // Each tag belongs to one record: the closest one before it. The same bytes read at other bit
    // shifts can also look like records (garbage max HP), and they sit farther from the tag.
    for tag in &tags {
        let Some((i, &(end, k, crit, hp, max, dmg, _, _))) = recs
            .iter()
            .enumerate()
            .filter(|(_, r)| r.0 <= tag.0 && tag.0 - r.0 <= EFFECT_TAG_SEARCH_BITS)
            .max_by_key(|(_, r)| r.0)
        else {
            continue;
        };
        used[i] = true;
        if tag.1 {
            continue; // damage you took
        }
        out.push(Hit {
            damage: dmg,
            attacker: 0,
            owner: 0,
            target: tag.3,
            crit,
            target_hp: hp,
            target_max_hp: max,
            at: (k, end / 8),
            effect: true,
            // Plain "DamageDone" = physical.
            element: Some(if tag.2.is_empty() {
                "Physical".into()
            } else {
                tag.2.clone()
            }),
            echo: false,
        });
    }
    // Area damage with no tag: the same fields, then ff ff ff ff and the owning player's id.
    // Dwarf in the Box's explosion and Mine send these, one per mob hit, no target id
    // (captures/2026-11-13_dwarf_mobs_box_mine.pcapng). Element from the kind byte as in hits.
    // The same record read at several bit shifts sits at different positions: dedupe by content.
    let mut seen = std::collections::HashSet::new();
    // Direct hits and tagged effects carry the same fields: never count those twice.
    for h in find_hits(msg).iter().chain(out.iter()) {
        seen.insert((h.target_hp, h.target_max_hp, h.damage));
    }
    for (i, &(end, k, crit, hp, max, dmg, kind, owner)) in recs.iter().enumerate() {
        let Some(owner) = owner else { continue };
        if used[i] || !seen.insert((hp, max, dmg)) {
            continue;
        }
        out.push(Hit {
            damage: dmg,
            attacker: owner,
            owner,
            target: 0,
            crit,
            target_hp: hp,
            target_max_hp: max,
            at: (k, end / 8),
            effect: true,
            element: Some(kind_element(kind).unwrap_or("Physical").to_string()),
            echo: false,
        });
    }
    out
}

/// Dwarf in the Box going off: the server spawns this aura ~0.9 s before the explosion's damage
/// records (captures/2026-11-13_dwarf_mobs_box_mine.pcapng). The aura is resent for ~2 s.
pub fn has_box_explosion(msg: &[u8]) -> bool {
    // "_aura", or "_poison_aura" with Master of Poison.
    const NAME: &[u8] = b"skill_jackinthebox_explosion_";
    if msg.len() < NAME.len() || msg[0] != SERVER_BUNDLE || msg.len() > MAX_HIT_MESSAGE {
        return false;
    }
    (0..8).any(|k| shifted(msg, k).windows(NAME.len()).any(|w| w == NAME))
}

/// Auras that mark a dwarf effect: (aura name part, source, element, how long after the aura its
/// damage records can come). The server spawns them just before the damage and resends them about
/// once a second while they last.
/// Frost: captures/2026-11-13_dwarf_ice_mastery.pcapng; fire: 2026-11-13_dwarf_fire_mastery.pcapng.
pub const EFFECT_MARKERS: &[(&[u8], &str, &str, f64)] = &[
    (
        b"skill_steamconductor_end_frost_explosion",
        "Steam Conductor",
        "Ice",
        0.4,
    ),
    (b"skill_grenade_frost_explosion", "Bomb", "Ice", 0.4),
    // Master of Ice level 2: a small frost explosion when you get hit.
    (
        b"dwarf_talent_frozen_master_frost_explosion",
        "Master of Ice",
        "Ice",
        0.4,
    ),
    // Master of Fire level 2: a fire explosion on hit (same damage on every mob, ~0.06 s later).
    (
        b"dwarf_talent_fire_explosion",
        "Master of Fire",
        "Fire",
        0.4,
    ),
    // Master of Fire burns: Rocket Pack's burning ground and Oil Slick's burning puddle. Burn
    // ticks come once a second while they last.
    (
        b"skill_hoverjump_burning_ground",
        "Rocket Pack",
        "Fire",
        1.2,
    ),
    (b"skill_trail_fire", "Oil Slick", "Fire", 1.2),
    // Dwarf in the Box with Master of Fire: its explosion record comes ~0.9 s after this aura.
    (
        b"skill_jackinthebox_explosion_fire",
        "Dwarf in the Box",
        "Fire",
        1.5,
    ),
    // Explosive Support's explosion on Rocket Pack landing / Iron Fist / Iron Pounce: Fire
    // records ~0.08 s later. (Its spawned bombs are found by their minion instead.)
    (
        b"dwarf_talent_explosive_damage",
        "Explosive Support",
        "Fire",
        0.4,
    ),
    // Master of Lightning level 2 (all classes): Lightning Strike's field and the field when
    // you're stunned. Resent ~1/s while it ticks (captures/2026-11-13_mage_lightning_mastery).
    (b"skill_lightningstrike_lightning_field", "Master of Lightning", "Lightning", 1.5),
    (b"all_talent_lightning_master_lightning_field", "Master of Lightning", "Lightning", 1.5),
    // Iron Drone (talent): the drone's explosion, Fire records ~0.3 s later.
    (
        b"dwarf_talent_irondwarf_minion_drone_explosion",
        "Iron Drone",
        "Fire",
        0.8,
    ),
    // Heavy Shot's explosion leaves a short burn.
    (b"skill_heavyshot_explosion", "Heavy Shot", "Fire", 0.4),
];

/// Sources (see EFFECT_MARKERS) whose aura is in this message.
pub fn find_effect_markers(msg: &[u8]) -> Vec<&'static str> {
    let mut out = Vec::new();
    if msg.len() < 30 || msg[0] != SERVER_BUNDLE || msg.len() > MAX_HIT_MESSAGE {
        return out;
    }
    for k in 0..8 {
        let x = shifted(msg, k);
        for (name, source, _, _) in EFFECT_MARKERS {
            if !out.contains(source) && x.windows(name.len()).any(|w| w == *name) {
                out.push(*source);
            }
        }
    }
    out
}

/// Spawn records. The name and the rank of one record sit at different bit alignments,
/// so both are found by absolute bit position and each rank takes the closest name before it.
pub fn find_spawns(msg: &[u8]) -> Vec<Spawn> {
    let mut out = Vec::new();
    if msg.len() < 20 || msg[0] != SERVER_BUNDLE || msg.len() > MAX_SPAWN_MESSAGE {
        return out;
    }
    let mut names: Vec<(usize, usize, String)> = Vec::new(); // (bit start, bit end, name)
    let mut ranks: Vec<(usize, String, u32)> = Vec::new(); // (bit start, rank, id)
    for k in 0..8u32 {
        let x = shifted(msg, k);
        for c in SPAWN.captures_iter(&x) {
            let len = u16::from_le_bytes([c[1][0], c[1][1]]) as usize;
            let start = c.get(1).unwrap().end();
            if !(4..=120).contains(&len) || start + len > x.len() {
                continue;
            }
            let name = &x[start..start + len];
            if is_ident(name) {
                let bit = start * 8 + k as usize;
                names.push((
                    bit,
                    bit + len * 8,
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
        }
        for m in RANK.find_iter(&x) {
            if let Some((rank, id)) = parse_rank(&x, m.start()) {
                ranks.push((m.start() * 8 + k as usize, rank, id));
            }
        }
    }
    if names.is_empty() {
        return out;
    }
    names.sort();
    ranks.sort();
    for (rpos, rank, id) in ranks {
        let best = names
            .iter()
            .filter(|(_, nend, _)| *nend <= rpos && rpos - nend <= RANK_SEARCH_BITS)
            .last();
        if let Some((_, _, name)) = best {
            out.push(Spawn {
                id,
                template: name.clone(),
                rank,
            });
        }
    }
    out
}

/// (player id, character name) pairs from player info messages.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub class: Option<&'static str>,
    pub level: Option<u16>,
}

pub fn find_names(msg: &[u8]) -> Vec<PlayerInfo> {
    let mut out: Vec<PlayerInfo> = Vec::new();
    if msg.len() < 12 || msg[0] != SERVER_BUNDLE || !matches!(msg[1], PLAYER_INFO | PLAYER_INFO_SHORT) || msg.len() > 8192 {
        return out;
    }
    for k in 0..8 {
        let x = shifted(msg, k);
        let mut at = 0;
        while let Some(c) = NAME.captures_at(&x, at) {
            at = c.get(0).unwrap().start() + 1;
            let start = c.get(0).unwrap().end() - 1;
            let len = c[2][0] as usize;
            let Some(raw) = x.get(start..start + len) else {
                continue;
            };
            let Ok(name) = std::str::from_utf8(raw) else {
                continue;
            };
            if !valid_name(name) {
                continue;
            }
            // Then [u16 len][zone], [u8 class], [u16 level]; or, in the short record, straight
            // [u8 class][u16 level] (class < 4 and level >= 1 can't read as a short zone length).
            let rest = &x[start + len..];
            let (class, level) = match rest
                .get(..2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            {
                Some(zl) if zl < 64 && rest.len() >= zl + 5 => {
                    let lvl = u16::from_le_bytes([rest[zl + 3], rest[zl + 4]]);
                    (
                        CLASSES.get(rest[zl + 2] as usize).copied(),
                        (1..=500).contains(&lvl).then_some(lvl),
                    )
                }
                Some(_) if rest.len() >= 3 => {
                    let lvl = u16::from_le_bytes([rest[1], rest[2]]);
                    match (CLASSES.get(rest[0] as usize).copied(), (1..=500).contains(&lvl)) {
                        (Some(c), true) => (Some(c), Some(lvl)),
                        _ => (None, None),
                    }
                }
                _ => (None, None),
            };
            let info = PlayerInfo {
                id: le32(&c[1]),
                name: name.to_string(),
                class,
                level,
            };
            if !out.contains(&info) {
                out.push(info);
            }
        }
    }
    out
}

/// minion_talent_frostnova_circle -> "Frostnova circle"
pub fn summon_name(template: &str) -> String {
    let t = template.strip_suffix("_scale").unwrap_or(template);
    let parts: Vec<&str> = t
        .split('_')
        .filter(|p| !matches!(*p, "minion" | "talent" | "skill") && !p.is_empty())
        .collect();
    let s = parts.join(" ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => template.to_string(),
    }
}

/// Skill id of a client skill-use message.
pub fn cast_skill(msg: &[u8]) -> Option<u16> {
    if msg.len() < 20
        || msg[0] != CLIENT_BUNDLE
        || !CAST_TYPES.contains(&msg[1])
        || msg[2..5] != [0, 0, 0]
    {
        return None;
    }
    Some(u16::from_le_bytes([msg[5], msg[6]]))
}

/// The big zone-load snapshot (sub-message 0x1d, ~1 MB) sent when you enter a zone.
/// Entity ids are reassigned from then on.
pub fn is_zone_load(msg: &[u8]) -> bool {
    msg.len() > 200_000 && msg[0] == SERVER_BUNDLE && msg[1] == 0x1d
}

/// Your own character in the zone-load message: name, class, and entity id when they read right.
/// Near the start: [u16 len][name] [u8 class] [9 bytes] [u16 len][zone, e.g. a0200_kingscity] [17 bytes]
/// [u32 id] (captures/2026-11-12_ranger_zone.pcapng, two zone loads).
pub fn zone_load_player(msg: &[u8]) -> Option<(String, Option<&'static str>, Option<u32>)> {
    if !is_zone_load(msg) {
        return None;
    }
    let u16_at = |i: usize| msg.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    for i in 40..400 {
        let Some(len) = u16_at(i).filter(|l| (3..=48).contains(l)) else { continue };
        let Some(raw) = msg.get(i + 2..i + 2 + len) else { continue };
        let Ok(name) = std::str::from_utf8(raw) else { continue };
        if !valid_name(name) {
            continue;
        }
        // The zone name follows shortly: that's what tells the name apart from other bytes.
        let after = i + 2 + len;
        let zone = (after..after + 16).find_map(|j| {
            let zl = u16_at(j).filter(|l| (5..=64).contains(l))?;
            let z = msg.get(j + 2..j + 2 + zl)?;
            z.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_').then_some(j + 2 + zl)
        });
        let Some(zone_end) = zone else { continue };
        let id = msg.get(zone_end + 17..zone_end + 21).map(le32).filter(|id| id >> 16 == 1);
        let class = msg.get(after).and_then(|c| CLASSES.get(*c as usize)).copied();
        return Some((name.to_string(), class, id));
    }
    None
}

pub fn is_cast(msg: &[u8]) -> bool {
    cast_skill(msg).is_some()
}

/// Server tick a client skill-use message was sent at (the u16 after the skill id and a float).
pub fn cast_tick(msg: &[u8]) -> Option<u16> {
    cast_skill(msg)?;
    Some(u16::from_le_bytes([msg[11], msg[12]]))
}

/// A cast the server relays to everyone nearby (captures/2026-11-12_party.pcapng).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayedCast {
    pub skill: u16,
    /// Copied from the caster's own message: matches `cast_tick` of your casts.
    pub tick: u16,
}

/// Server 0x5f messages carry each nearby player's skill use verbatim:
/// `ff [46..4b] 00 00 00 [u16 skill][f32][u16 tick] 00 00`. Your own casts come back too.
/// The record names no caster; tell yours apart by tick, others by class (see Meter).
static RELAYED_CAST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)\xff[\x46-\x4b]\x00\x00\x00(..).{4}(..)\x00\x00").unwrap());
const RELAY_SUB: u8 = 0x5f;

pub fn find_relayed_casts(msg: &[u8]) -> Vec<RelayedCast> {
    let mut out: Vec<RelayedCast> = Vec::new();
    if msg.len() < 20
        || msg[0] != SERVER_BUNDLE
        || msg[1] != RELAY_SUB
        || msg.len() > MAX_HIT_MESSAGE
    {
        return out;
    }
    for k in 0..8 {
        let x = shifted(msg, k);
        for c in RELAYED_CAST.captures_iter(&x) {
            let skill = u16::from_le_bytes([c[1][0], c[1][1]]);
            let tick = u16::from_le_bytes([c[2][0], c[2][1]]);
            let rc = RelayedCast { skill, tick };
            if SKILLS.iter().any(|s| s.0 == skill) && !out.contains(&rc) {
                out.push(rc);
            }
        }
    }
    out
}

/// "Spellweaver" / "Ranger" for a known skill id.
pub fn skill_class(id: u16) -> Option<&'static str> {
    match id {
        0x073f..=0x0757 => Some("Spellweaver"),
        0x0758..=0x076a => Some("Ranger"),
        0x072e..=0x073e => Some("Dragonknight"),
        0x076b..=0x077f => Some("Steam Mechanicus"),
        _ => None,
    }
}

/// Skill ids seen in casts (captures/2026-11-11_skills.pcapng, used in a known order).
/// `deals_damage` = false for skills whose hits should not be credited to them.
const SKILLS: &[(u16, &str, bool)] = &[
    (0x073f, "Magic Missile", true),
    (0x0740, "Fireball", true),
    (0x0741, "Ice Missile", true),
    (0x0742, "Frozen Sphere", true),
    (0x0743, "Frost Nova", true),
    (0x0744, "Teleport", false),
    (0x0745, "Lightning Strike", true),
    (0x074a, "Frost Wind", true),
    (0x074b, "Chain Lightning", true),
    (0x074d, "Guardian", true),
    (0x074e, "Guardian of Flames", true),
    (0x074f, "Destruction", true),
    (0x0750, "Singularity", true),
    (0x0752, "Meteor", true),
    (0x0753, "Mind Control", false),
    (0x0755, "Frost Charge", true),
    (0x0756, "Fire Blast", true),
    (0x0757, "Ball Lightning", true),
    // Cast once, then an aura around you deals Poison damage (effect records) for ~15 s.
    // `false` so your direct hits right after casting it aren't credited to it.
    (0x0751, "Toxic Explosion", false),
    // Ranger (captures/2026-11-12_ranger.pcapng, cast one by one in a known order).
    (0x0758, "Death Sweep", true),
    (0x075a, "Hunting Arrow", true),
    (0x075b, "Blade Dance", true),
    (0x075c, "Hunting Trap", true),
    (0x075d, "Precise Shot", true),
    (0x075e, "Explosive Arrow", true),
    (0x075f, "Net", false),
    (0x0760, "Dive", false),
    (0x0761, "Adrenaline", false),
    (0x0762, "Bird of Prey", true),
    (0x0763, "Scatter Shot", true),
    (0x0764, "Chimera Scatter", true),
    (0x0765, "Wild Pack", true),
    (0x0766, "Deadly Blow", true),
    (0x0768, "Thicket of Thorns", true),
    (0x0769, "Hornet Cloud", true),
    (0x076a, "Tree of Life", false),
    // Dragonknight (captures/2026-11-12_dragonknight.pcapng, cast one by one in a known order).
    (0x072e, "Rage Attack", true),
    // 073a/073c fixed by captures/2026-11-12_dragonknight_passives.pcapng: Agathon's Guard
    // (a Banner of War talent) summons on 073a, Unleashed Dragon (a Fury of the Dragon talent)
    // rains on 073c. 072f/0730/0735 corrected by the user casting each in game.
    (0x072f, "Rageful Swing", true),
    (0x0730, "Bloody Wild Swing", true),
    (0x0732, "Mighty Wild Swing", true),
    (0x0733, "Charge", true),
    (0x0734, "Furious Battle Cry", false),
    // No direct damage; with Lightning Mastery it deals Lightning effect damage (see TALENT_PROCS).
    (0x0735, "Outburst", false),
    (0x0736, "Dragon Hide", false),
    (0x0737, "Ground Breaker", true),
    (0x0738, "Iron Brow", true),
    (0x0739, "Rage Jump", true),
    // Defiance buff auras; summons Agathon's Guard with that talent.
    (0x073a, "Banner of War", false),
    // Spawns skill_earthquake_*; its damage comes as Fire effect ticks once a second for ~8 s.
    (0x073c, "Fury of the Dragon", true),
    // The Chimera version of Bloody Wild Swing: a stacking Poison DoT (effect records).
    (0x0731, "Chimera Bloody Wild Swing", true),
    (0x073b, "Smash", true),
    (0x073e, "Spike Shield", false),
    // Steam Mechanicus (captures/2026-11-13_dwarf.pcapng). Named from what each cast makes the
    // server spawn ~0.1-0.5 s later (auras, minions) and the hits that follow; not from a known
    // cast order yet. "Skill xxxx" ones are unidentified, waiting on the user's feedback.
    (0x076f, "Heavy Shot", true), // skill_heavyshot_explosion_aura + physical hit
    (0x0778, "Dwarf in the Box", true), // minion_jackinthebox; explodes ~5 s later (Fire effect)
    (0x077c, "Flamethrower", true), // Fire hits on several targets, re-sent while held
    (0x077a, "Rocket Pack", false), // skill_hoverjump_buff_damage_aura
    (0x0772, "Bomb", true),       // skill_grenade_damage_aura
    (0x0771, "Mine", true),       // dwarf_mine
    (0x0773, "Oil Slick", false), // skill_trail_aura
    (0x0774, "Mechanical Turret", false), // minion_combatTurret; user-confirmed by its tooltip (blue, lasts 10 s)
    (0x0779, "Tactical Turret", false), // minion_coveringfireturret
    (0x0775, "Tesla Turret", false), // minion_teslaturret
    (0x0776, "Machine Gun Turret", false), // minion_machinegunturret
    (0x077b, "Shrapnel Shot", true), // probable: Fire hit with skill_ShrapnelShot_damage_aura
    (0x076e, "Chimera Shot", true), // probable: Poison hits + stacking Poison ticks
    (0x0770, "C14 Micro Rocket", true), // user-confirmed
    (0x077e, "Iron Fist", true),  // user-confirmed
    (0x077f, "Iron Pounce", true), // user-confirmed
    (0x076d, "Quick Shot", true), // probable: physical hit; 076e (Chimera Shot) is its Chimera version
    (0x0777, "Steam Conductor", false), // user-confirmed
    (0x076c, "Iron Dwarf", false), // user-confirmed
    (0x076b, "Stop Iron Dwarf", false), // user-confirmed: leaves Iron Dwarf form
];

pub fn skill_name(id: u16) -> String {
    SKILLS
        .iter()
        .find(|s| s.0 == id)
        .map(|s| s.1.to_string())
        .unwrap_or_else(|| format!("Skill {id:04x}"))
}

pub fn skill_deals_damage(id: u16) -> bool {
    SKILLS.iter().find(|s| s.0 == id).is_none_or(|s| s.2)
}

/// Element of a skill's direct hits (only where it is unambiguous from the skill).
pub fn skill_element(skill: &str) -> Option<&'static str> {
    Some(match skill {
        "Fireball" | "Meteor" | "Fire Blast" | "Guardian of Flames" => "Fire",
        "Ice Missile" | "Frozen Sphere" | "Frost Nova" | "Frost Wind" | "Frost Charge" => "Ice",
        "Lightning Strike" | "Chain Lightning" | "Ball Lightning" => "Lightning",
        "Toxic Explosion" | "Chimera Shot" => "Poison",
        _ => return None,
    })
}

/// Skills whose follow-up effects (burns, explosions, talent procs) deal damage of an element,
/// most likely source first. Used to credit effect damage.
/// Talents that add damage to a skill's hits, reported as effect records without a name:
/// (skill, element of the records, talent name). The record comes ~0.2 s after the skill's cast.
/// Exploding Hit: its tooltip says physical, but the server tags it DamageDoneFire
/// (captures/2026-11-12_exploding_hit.pcapng; same crit ratio as Deadly Blow's own hits).
/// Outburst deals nothing by default; with Lightning Mastery each cast spawns
/// warrior_talent_lightning_master_lightning_explosion_aura and two DamageDoneLightning records
/// ~0.4 s later (captures/2026-11-12_dk_outburst_mastery.pcapng). Credited to Outburst itself.
pub const TALENT_PROCS: &[(&str, &str, &str)] = &[
    ("Deadly Blow", "Fire", "Exploding Hit"),
    ("Outburst", "Lightning", "Outburst"),
    ("Heavy Shot", "Fire", "Heavy Shot"),
    ("C14 Micro Rocket", "Poison", "C14 Micro Rocket"),
];

pub fn effect_skills(element: &str) -> &'static [&'static str] {
    match element {
        "Fire" => &[
            "Fireball",
            "Meteor",
            "Fire Blast",
            "Guardian of Flames",
            "Explosive Arrow",
            "Fury of the Dragon",
            // Dwarf: Bomb's Fire records come ~1.8 s after the cast (2026-11-13_dwarf_box_bomb_mine2).
            // The box last: its own fuse ticks are damage to the box, and on Grimmag it was killed
            // before its explosion did any damage.
            "Bomb",
            "Mine",
            "Flamethrower",
            "Shrapnel Shot",
            "Dwarf in the Box",
        ],
        "Ice" => &[
            "Frost Wind",
            "Frost Charge",
            "Frozen Sphere",
            "Frost Nova",
            "Ice Missile",
        ],
        "Lightning" => &["Ball Lightning", "Chain Lightning", "Lightning Strike"],
        // Ranger poison: Hunting Trap leaves a DoT; Chimera Scatter adds a little.
        "Poison" => &[
            "Toxic Explosion",
            "Hunting Trap",
            "Chimera Scatter",
            "Chimera Bloody Wild Swing",
            "Chimera Shot",
        ],
        _ => &[],
    }
}

/// In-game names of mercenaries, by spawn template (`mercenary_ancient_watcher`).
const MERCENARIES: &[(&str, &str)] = &[
    ("viridis", "Viridis the Sylvan Treant"),
    ("ember_knight_kain", "Ember Knight Kain"),
    ("divine_wanderer", "Sacred Expedition"),
    ("ancient_watcher", "Blade Sentinel"),
    ("shadow_stalker", "Twilight Wrath"),
];

/// Display name of a mercenary spawn template; unknown mercenaries get a cleaned-up template name.
pub fn mercenary_name(template: &str) -> Option<String> {
    let key = template.strip_prefix("mercenary_")?;
    let key = key.strip_suffix("_scale").unwrap_or(key);
    Some(match MERCENARIES.iter().find(|(k, _)| *k == key) {
        Some((_, name)) => name.to_string(),
        None => summon_name(template),
    })
}

/// Summons that belong to a skill, so their damage is shown under it.
pub fn summon_skill(template: &str) -> Option<&'static str> {
    [
        ("frostnova_circle", "Frost Nova"),
        ("firemanifestation", "Guardian of Flames"),
        ("arcanemanifestation", "Guardian"),
        ("packofwolves", "Wild Pack"),
        ("summontree", "Tree of Life"),
        // minion_talent_uniskel_warrior_agathon_01..03, from Banner of War's talent.
        ("warrior_agathon", "Agathon's Guard"),
        // Steam Mechanicus turrets and the box (minion_combatTurret, minion_jackinthebox...).
        ("combatTurret", "Mechanical Turret"),
        ("coveringfireturret", "Tactical Turret"),
        ("teslaturret", "Tesla Turret"),
        ("machinegunturret", "Machine Gun Turret"),
        ("jackinthebox", "Dwarf in the Box"),
        ("dwarf_mine", "Mine"),
        // Iron Drone talent: kills in Iron Dwarf form summon drones that explode ("Irondwarf
        // dwarven drone" in the meter before, from its template).
        ("dwarven_drone", "Iron Drone"),
        // Master of Poison level 2: on a kill, a chance to spawn 3 poison bullets circling for
        // 10 s ("Poison orbital field" in the meter before; template seen by the user live).
        ("poison_orbital_field", "Master of Poison"),
        // Jewels that summon a fighter.
        ("jewel_buff_snow_wolf", "Jewel of Glacial Fang"),
    ]
    .iter()
    .find(|(k, _)| template.contains(k))
    .map(|(_, v)| *v)
}

/// a0301_normal_g_gen_thing_swamp_scale -> "thing swamp",
/// c2303_miniboss_medusa_05_scale -> "miniboss medusa",
/// boss_medusa_normal_add_gorgon_scale -> "boss medusa add gorgon"
pub fn pretty_name(template: &str) -> String {
    let t = template.strip_suffix("_scale").unwrap_or(template);
    let mut parts: Vec<&str> = t.split('_').collect();
    if let Some(i) = parts.iter().position(|p| *p == "gen") {
        parts.drain(..=i);
    }
    // zone code like a0301 / c2303
    if parts.first().is_some_and(|p| {
        p.len() == 5
            && p.as_bytes()[0].is_ascii_alphabetic()
            && p[1..].bytes().all(|b| b.is_ascii_digit())
    }) {
        parts.remove(0);
    }
    parts.retain(|p| !matches!(*p, "normal" | "g") && !p.bytes().all(|b| b.is_ascii_digit()));
    let s = parts.join(" ");
    if s.is_empty() {
        template.to_string()
    } else {
        s
    }
}

/// Rank id -> category used by the client: worldboss, boss, leader, champion, guardian,
/// add (boss summons), minion (player summons), monster. e.g. Rank_boss_endlesstower -> boss.
pub fn rank_category(rank: &str) -> String {
    let Some(rid) = rank.strip_prefix("Rank_") else {
        return "unknown".into();
    };
    let rid = rid.to_ascii_lowercase();
    for cat in [
        "worldboss",
        "boss",
        "leader",
        "champion",
        "guardian",
        "add",
        "minion",
        "monster",
    ] {
        if rid == cat || rid.starts_with(&format!("{cat}_")) || rid.ends_with(&format!("_{cat}")) {
            return cat.into();
        }
    }
    rid
}
