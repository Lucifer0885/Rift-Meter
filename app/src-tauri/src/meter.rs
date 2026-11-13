//! Encounter bookkeeping: totals, DPS and crit rate per attacker, mob types, history.

use crate::decode::{Hit, PlayerInfo, RelayedCast, Spawn, skill_class, TALENT_PROCS, effect_skills, mercenary_name, skill_element, pretty_name, rank_category, skill_deals_damage, skill_name, summon_name, summon_skill};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Hits this soon after our own cast count as votes for "which entity is me".
const CAST_WINDOW: f64 = 1.5;
const ME_VOTES: usize = 30;
const RECENT_HITS: usize = 60;
const HISTORY: usize = 20;
/// Direct hits this soon after a damaging cast are credited to that skill.
const SKILL_WINDOW: f64 = 3.0;
/// Casts this long before a fight's first hit still count for that fight.
const PRE_FIGHT_CASTS: f64 = 5.0;
/// Effect damage (burns, explosions, auras like Toxic Explosion's) is credited to a matching skill
/// cast this recently.
const EFFECT_WINDOW: f64 = 20.0;
/// A talent's effect record follows its skill's cast by ~0.2 s.
const TALENT_PROC_WINDOW: f64 = 1.0;
pub const OTHER_SOURCE: &str = "Other hits";
/// Row for Rift Echo hits (Rift Force arcane vessel).
pub const RIFT_FORCE: &str = "Rift Force";
/// Summon whose spawn we didn't see (it was out before capture started): almost always the mercenary.
pub const UNKNOWN_MERCENARY: &str = "Mercenary";
/// Mage skill ids (Toxic Explosion is the mage's only poison damage).
const MAGE_SKILLS: std::ops::RangeInclusive<u16> = 0x073f..=0x0757;
const RANGER_SKILLS: std::ops::RangeInclusive<u16> = 0x0758..=0x076a;
const DK_SKILLS: std::ops::RangeInclusive<u16> = 0x072e..=0x073e;
/// Steam Mechanicus Mine's skill id.
const MINE_SKILL: u16 = 0x0771;
const BOMB_SKILL: u16 = 0x0772;
const BOMB_WINDOW: f64 = 3.0;
/// Dwarf in the Box's damage comes ~0.9 s after its explosion aura; the aura is resent for ~2 s.
const BOX_EXPLOSION_WINDOW: f64 = 3.0;
/// Area damage (owner-tagged, no skill) we couldn't place.
const AREA_SOURCE: &str = "Area damage";
/// Ids at or above this are made up for targets whose id we never saw.
const UNNAMED_TARGET: u32 = 0xf000_0000;

#[derive(Default, Clone)]
struct Tally {
    damage: u64,
    hits: u32,
    crits: u32,
    max_hit: u32,
    /// Damage from effects that name an element (burns, frost explosions, ...).
    elements: HashMap<String, u64>,
}

impl Tally {
    fn add(&mut self, dmg: u32, crit: bool, element: Option<&str>) {
        self.damage += dmg as u64;
        self.hits += 1;
        self.crits += crit as u32;
        self.max_hit = self.max_hit.max(dmg);
        if let Some(e) = element {
            *self.elements.entry(e.to_string()).or_default() += dmg as u64;
        }
    }
    fn element_views(&self) -> Vec<ElementView> {
        let mut v: Vec<ElementView> =
            self.elements.iter().map(|(name, damage)| ElementView { name: name.clone(), damage: *damage }).collect();
        v.sort_by(|a, b| b.damage.cmp(&a.damage));
        v
    }
    fn crit_rate(&self) -> f64 {
        if self.hits > 0 { self.crits as f64 / self.hits as f64 } else { 0.0 }
    }
}

#[derive(Default, Clone)]
struct AttackerStats {
    total: Tally,
    /// Per skill (or summon type for summons we can't tie to a skill).
    sources: HashMap<String, Tally>,
    /// Skill uses; only known for your own character.
    casts: HashMap<String, u32>,
}

#[derive(Clone)]
struct Encounter {
    start: f64,
    last: f64,
    attackers: HashMap<u32, AttackerStats>,
    targets: HashMap<(String, String), u64>, // (name, rank) -> damage
    /// Boss killed: (time, name). The timer stops there.
    killed: Option<(f64, String)>,
    /// Already handed out by `take_finished` (written to disk).
    saved: bool,
}

impl Encounter {
    fn duration(&self) -> f64 {
        (self.killed.as_ref().map_or(self.last, |k| k.0) - self.start).max(1.0)
    }
}

/// Damage this soon after a boss kill (burns, last hits on adds) still goes to the boss fight.
const AFTER_KILL_GRACE: f64 = 5.0;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ElementView {
    pub name: String,
    pub damage: u64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceView {
    pub name: String,
    pub damage: u64,
    pub dps: f64,
    pub hits: u32,
    pub crits: u32,
    pub crit_rate: f64,
    pub max_hit: u32,
    /// Share of this attacker's damage.
    pub share: f64,
    /// Times you used this skill (your own character only).
    pub casts: u32,
    pub elements: Vec<ElementView>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AttackerView {
    pub id: u32,
    pub name: String,
    pub is_me: bool,
    pub damage: u64,
    pub dps: f64,
    pub hits: u32,
    pub crits: u32,
    pub crit_rate: f64,
    pub max_hit: u32,
    pub share: f64,
    pub sources: Vec<SourceView>,
    pub elements: Vec<ElementView>,
    /// "Ranger", "Spellweaver", ... (from zone entry, or for you from your skills).
    pub class: Option<String>,
    pub level: Option<u16>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TargetView {
    pub name: String,
    pub rank: String,
    pub damage: u64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EncounterView {
    pub start: f64,
    pub duration: f64,
    pub over: bool,
    /// Name of the boss whose death ended the fight.
    pub killed: Option<String>,
    pub total: u64,
    pub attackers: Vec<AttackerView>,
    pub targets: Vec<TargetView>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitView {
    pub time: f64,
    pub damage: u32,
    pub crit: bool,
    pub attacker: u32,
    pub owner: u32,
    pub is_me: bool,
    pub target: u32,
    pub target_name: String,
    pub target_rank: String,
    pub counted: bool,
}

/// HP state of one target, from the HP fields carried by every hit on it.
#[derive(Clone)]
struct TargetState {
    hp: u64,
    max_hp: u64,
    first_hit: f64,
    last_hit: f64,
    /// Damage from everyone (you, party, pets) since we first saw it.
    damage: u64,
    my_damage: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetHpView {
    pub id: u32,
    pub name: String,
    pub rank: String,
    pub hp: u64,
    pub max_hp: u64,
    pub pct: f64,
    pub dead: bool,
    /// Everyone's DPS on this target.
    pub dps: f64,
    pub my_damage: u64,
    /// Seconds until it dies at the current DPS (alive targets only).
    pub time_to_kill: Option<f64>,
    pub fight_time: f64,
    pub since_last_hit: f64,
}

/// Higher = more important; picks which target gets the HP bar.
fn rank_priority(rank: &str) -> u8 {
    match rank {
        "worldboss" => 8,
        "boss" => 7,
        "leader" => 6,
        "champion" => 5,
        "guardian" => 4,
        "add" => 3,
        "monster" => 2,
        "unknown" => 1,
        _ => 0, // minion (player summons)
    }
}

const ELITE_PRIORITY: u8 = 4; // guardian and up
const TARGET_MEMORY: f64 = 120.0; // forget targets not hit for this long
const MAX_TRACKED: usize = 8;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub me: Option<u32>,
    pub encounter: Option<EncounterView>,
    /// The most important target hit recently (bosses first).
    pub focus_target: Option<TargetHpView>,
    /// Recently hit targets with HP, most important first.
    pub tracked_targets: Vec<TargetHpView>,
    pub recent_hits: Vec<HitView>,
    pub history: Vec<EncounterView>,
    pub known_entities: usize,
}

pub struct Meter {
    pub idle_timeout: f64,
    /// End the fight (stop the timer) when a boss dies.
    pub stop_on_boss_kill: bool,
    /// Rank categories to count; None = all.
    pub ranks: Option<Vec<String>>,
    pub include_unknown: bool,
    encounter: Option<Encounter>,
    history: VecDeque<Encounter>,
    /// Finished fights not yet handed out by `take_finished`.
    unsaved: Vec<EncounterView>,
    entities: HashMap<u32, (String, String)>, // id -> (template, rank id)
    names: HashMap<u32, String>,              // player id -> character name
    classes: HashMap<u32, (Option<&'static str>, Option<u16>)>, // player id -> class, level
    /// Your class from the skills you cast (for when no zone entry was captured).
    my_class: Option<&'static str>,
    /// Your mercenary's name, once one of its spawns was seen. Kept across zones: the same
    /// mercenary comes back after each zone change, often spawned before we can see it.
    my_mercenary: Option<String>,
    /// Element -> (time, skill) of your last direct hit of that element. A DoT of that element
    /// with no matching skill of its own (e.g. poison mastery's stacking poison) goes to it.
    element_hits: HashMap<String, (f64, String)>,
    last_cast: Option<f64>,
    /// Your recent casts: (time, skill id).
    casts: VecDeque<(f64, u16)>,
    /// (skill, tick) of your recent casts, to spot the server relaying them back.
    my_ticks: VecDeque<(u16, u16)>,
    /// Party members' recent casts, from the server's relayed casts: player id -> (time, skill id).
    party_casts: HashMap<u32, VecDeque<(f64, u16)>>,
    me_votes: VecDeque<u32>,
    /// Effect hits that arrived before we knew which player is you (they name no attacker).
    /// Counted once a direct hit tells us; see on_hit.
    pending_effects: Vec<(f64, Hit)>,
    /// Last time a Dwarf in the Box explosion aura was seen.
    box_explosion: Option<f64>,
    recent: VecDeque<HitView>,
    targets: HashMap<u32, TargetState>,
    /// Every hit decoded so far, counted or not.
    pub hits_seen: u64,
}

impl Meter {
    pub fn new() -> Self {
        Self {
            idle_timeout: 30.0,
            stop_on_boss_kill: true,
            ranks: None,
            include_unknown: false,
            encounter: None,
            history: VecDeque::new(),
            unsaved: Vec::new(),
            entities: HashMap::new(),
            names: HashMap::new(),
            classes: HashMap::new(),
            my_class: None,
            my_mercenary: None,
            element_hits: HashMap::new(),
            last_cast: None,
            casts: VecDeque::new(),
            my_ticks: VecDeque::new(),
            party_casts: HashMap::new(),
            me_votes: VecDeque::new(),
            pending_effects: Vec::new(),
            box_explosion: None,
            recent: VecDeque::new(),
            targets: HashMap::new(),
            hits_seen: 0,
        }
    }

    pub fn me(&self) -> Option<u32> {
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for v in &self.me_votes {
            *counts.entry(*v).or_default() += 1;
        }
        counts.into_iter().max_by_key(|(id, c)| (*c, std::cmp::Reverse(*id))).map(|(id, _)| id)
    }

    pub fn on_cast(&mut self, t: f64, skill: u16, tick: u16) {
        self.last_cast = Some(t);
        self.casts.push_back((t, skill));
        self.my_ticks.push_back((skill, tick));
        if self.my_ticks.len() > 64 {
            self.my_ticks.pop_front();
        }
        if MAGE_SKILLS.contains(&skill) {
            self.my_class = Some("Spellweaver");
        } else if RANGER_SKILLS.contains(&skill) {
            self.my_class = Some("Ranger");
        } else if DK_SKILLS.contains(&skill) {
            self.my_class = Some("Dragonknight");
        } else if skill_class(skill) == Some("Steam Mechanicus") {
            self.my_class = Some("Steam Mechanicus");
        }
        while self.casts.front().is_some_and(|c| t - c.0 > 60.0) {
            self.casts.pop_front();
        }
        // Count it in the running fight (casts before a fight are added when it starts).
        let idle = self.idle_timeout;
        if let (Some(me), Some(enc)) = (self.me(), self.encounter.as_mut()) {
            if t - enc.last <= idle {
                *enc.attackers.entry(me).or_default().casts.entry(skill_name(skill)).or_default() += 1;
            }
        }
    }

    /// A skill use the server relayed. It names no caster: skip your own (same skill and tick as
    /// one you sent), then credit it to the one other player in the zone of that skill's class.
    /// With two players of that class it can't be told apart, so it's dropped.
    pub fn on_relayed_cast(&mut self, t: f64, c: RelayedCast) {
        if self.my_ticks.contains(&(c.skill, c.tick)) {
            return;
        }
        let (Some(me), Some(class)) = (self.me(), skill_class(c.skill)) else { return };
        let mut who = self.classes.iter().filter(|(id, (cl, _))| **id != me && *cl == Some(class)).map(|(id, _)| *id);
        let (Some(id), None) = (who.next(), who.next()) else { return };
        let q = self.party_casts.entry(id).or_default();
        q.push_back((t, c.skill));
        while q.front().is_some_and(|c| t - c.0 > 60.0) {
            q.pop_front();
        }
        let idle = self.idle_timeout;
        if let Some(enc) = self.encounter.as_mut() {
            if t - enc.last <= idle {
                *enc.attackers.entry(id).or_default().casts.entry(skill_name(c.skill)).or_default() += 1;
            }
        }
    }

    /// Burns, explosions and talent procs: the most likely skill of that element cast recently.
    fn effect_source(&self, t: f64, element: Option<&str>) -> String {
        let Some(element) = element else {
            return self.recent_skill(t).unwrap_or_else(|| OTHER_SOURCE.into());
        };
        let recent_within = |name: &str, window: f64| {
            self.casts.iter().rev().find(|(ct, id)| *ct <= t && t - ct <= window && skill_name(*id) == name).map(|c| c.0)
        };
        // A talent proc right after its skill's cast wins over skills of the same element.
        if let Some((_, _, talent)) =
            TALENT_PROCS.iter().find(|(skill, el, _)| *el == element && recent_within(skill, TALENT_PROC_WINDOW).is_some())
        {
            return talent.to_string();
        }
        let recent = |name: &str| recent_within(name, EFFECT_WINDOW);
        effect_skills(element)
            .iter()
            .find(|s| recent(s).is_some())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Toxic Explosion's skill id isn't known yet, so its cast can't be matched; but a
                // mage's only poison damage is Toxic Explosion.
                if let Some((ht, skill)) = self.element_hits.get(element) {
                    if *ht <= t + 0.05 && t - ht <= EFFECT_WINDOW {
                        return skill.clone();
                    }
                }
                let mage = self.casts.iter().any(|(_, id)| MAGE_SKILLS.contains(id));
                match element {
                    "Poison" if mage => "Toxic Explosion".into(),
                    // A frost DoT ticking once a second; with no Ice skill of yours behind it
                    // (rangers have none) it's Jewel of Eternal Scorn's ice field.
                    "Ice" if !mage => "Jewel of Eternal Scorn".into(),
                    // Dragonknights have no Lightning skill: it's the lightning set's field
                    // (SetLight2026_warrior_lightning_field_aura).
                    "Lightning" if self.my_class == Some("Dragonknight") => "Lightning Field".into(),
                    _ => format!("{element} effects"),
                }
            })
    }

    pub fn on_box_explosion(&mut self, t: f64) {
        self.box_explosion = Some(t);
    }

    /// Untagged area damage that names its owner: Dwarf in the Box right after its explosion
    /// aura, otherwise a Mine the owner placed (mines can wait a long time for a mob).
    fn area_source(&self, t: f64, owner: u32, mine: bool) -> String {
        if self.box_explosion.is_some_and(|b| (0.0..=BOX_EXPLOSION_WINDOW).contains(&(t - b))) {
            return "Dwarf in the Box".into();
        }
        let casts = if mine { Some(&self.casts) } else { self.party_casts.get(&owner) };
        // Bomb's explosion lands ~1.8 s after the cast, together with its Fire record.
        if casts.is_some_and(|q| q.iter().any(|(ct, id)| *id == BOMB_SKILL && *ct <= t && t - ct <= BOMB_WINDOW)) {
            return "Bomb".into();
        }
        if casts.is_some_and(|q| q.iter().any(|(ct, id)| *id == MINE_SKILL && *ct <= t && t - ct <= 60.0)) {
            return "Mine".into();
        }
        AREA_SOURCE.into()
    }

    /// The damaging skill you cast most recently before `t`.
    fn recent_skill(&self, t: f64) -> Option<String> {
        recent_damaging(&self.casts, t)
    }
}

/// The damaging skill in `casts` used most recently before `t`.
fn recent_damaging(casts: &VecDeque<(f64, u16)>, t: f64) -> Option<String> {
    {
        casts
            .iter()
            .rev()
            .filter(|(ct, id)| *ct <= t + 0.05 && t - ct <= SKILL_WINDOW && skill_deals_damage(*id))
            .map(|(_, id)| skill_name(*id))
            .next()
    }
}

impl Meter {

    pub fn on_spawn(&mut self, s: Spawn) {
        self.entities.insert(s.id, (s.template, s.rank));
    }

    /// A spawned monster (any rank except player summons).
    fn is_mob(&self, id: u32) -> bool {
        self.entities.get(&id).is_some_and(|(_, rank)| rank_category(rank) != "minion")
    }

    /// New zone: entity ids are reused for different things, so forget them and end the fight.
    pub fn on_zone_change(&mut self) {
        self.finish();
        self.entities.clear();
        self.names.clear();
        self.classes.clear();
        self.party_casts.clear();
        self.targets.clear();
        self.me_votes.clear();
        self.pending_effects.clear();
    }

    pub fn on_player(&mut self, p: PlayerInfo) {
        self.names.insert(p.id, p.name);
        if p.class.is_some() {
            self.classes.insert(p.id, (p.class, p.level));
        }
    }

    /// Class and level of a player, if known.
    fn player_class(&self, id: u32, is_me: bool) -> (Option<String>, Option<u16>) {
        match self.classes.get(&id) {
            Some((c, l)) => (c.map(String::from), *l),
            None if is_me => (self.my_class.map(String::from), None),
            None => (None, None),
        }
    }

    /// Character name if the server sent it, else a placeholder.
    pub fn player_name(&self, id: u32) -> String {
        match self.names.get(&id) {
            Some(n) => n.clone(),
            None if self.me() == Some(id) => "You".into(),
            None => format!("Player {:04x}", id & 0xffff),
        }
    }

    /// Which skill a hit belongs to. Summons map to the skill that creates them; your direct hits
    /// go to your last damaging cast, party members' to theirs (from relayed casts).
    fn source_name(&self, t: f64, hit: &Hit, mine: bool) -> String {
        // Rift Force's echo of any hit (yours, your mercenary's or your summons'): its own row.
        if hit.echo {
            return RIFT_FORCE.into();
        }
        // Area records come with their owner as attacker; tagged effects have none.
        if hit.effect && hit.attacker != 0 {
            return self.area_source(t, hit.owner, mine);
        }
        if hit.effect {
            return self.effect_source(t, hit.element.as_deref());
        }
        if hit.attacker != hit.owner {
            return match self.entities.get(&hit.attacker) {
                Some((template, _)) => mercenary_name(template)
                    .or_else(|| summon_skill(template).map(String::from))
                    .unwrap_or_else(|| summon_name(template)),
                None if mine => self.my_mercenary.clone().unwrap_or_else(|| UNKNOWN_MERCENARY.into()),
                None => UNKNOWN_MERCENARY.into(),
            };
        }
        let skill = if mine {
            self.recent_skill(t)
        } else {
            self.party_casts.get(&hit.owner).and_then(|q| recent_damaging(q, t))
        };
        skill.unwrap_or_else(|| OTHER_SOURCE.into())
    }

    /// (readable name, rank category) of an entity.
    pub fn target_label(&self, id: u32) -> (String, String) {
        match self.entities.get(&id) {
            Some((template, rank)) => (pretty_name(template), rank_category(rank)),
            None => ("unknown".into(), "unknown".into()),
        }
    }

    fn counts(&self, rank: &str) -> bool {
        match &self.ranks {
            None => true,
            Some(r) if r.is_empty() => true,
            Some(r) => r.iter().any(|x| x == rank) || (rank == "unknown" && self.include_unknown),
        }
    }

    /// A mercenary or a summon from a player skill (minion_packofwolves, minion_summontree...).
    /// Mob summons are named after their boss (boss_grimmag_minion_...), so they don't match.
    fn is_player_summon(&self, id: u32) -> bool {
        self.entities.get(&id).is_some_and(|(tpl, _)| tpl.starts_with("mercenary_") || tpl.starts_with("minion_"))
    }

    pub fn on_hit(&mut self, t: f64, hit: Hit) {
        self.on_hit_inner(t, hit);
        // You're known now: count the effect hits that came first (a Bomb opening a fight).
        if !self.pending_effects.is_empty() && self.me().is_some() {
            for (pt, ph) in std::mem::take(&mut self.pending_effects) {
                self.on_hit_inner(pt, ph);
            }
        }
    }

    fn on_hit_inner(&mut self, t: f64, mut hit: Hit) {
        let mut retargeted = false;
        if self.is_player_summon(hit.target) {
            // Damage your summons take: mob hits, and burns (effect records have no attacker).
            let from_us = self.is_player_summon(hit.attacker) || (hit.owner != 0 && Some(hit.owner) == self.me());
            if hit.effect || !from_us {
                return;
            }
            // Summon hits sometimes carry a summon's id in the target slot while the HP fields
            // are the real target's (seen with Wild Pack on Grimmag): find it by max HP instead.
            hit.target = 0;
            retargeted = true;
        }
        if hit.target == 0 {
            // No target id in the record: use the recently hit target with the same max HP.
            let found = self
                .targets
                .iter()
                .filter(|(_, ts)| ts.max_hp == hit.target_max_hp && t - ts.last_hit < self.idle_timeout)
                .max_by(|a, b| a.1.last_hit.total_cmp(&b.1.last_hit).then(a.0.cmp(b.0)))
                .map(|(id, _)| *id);
            // Not seen yet (e.g. pets, whose hits never carry the target id): track it under a
            // made-up id per max HP so its damage and HP still count.
            // A hit on your own summon with no matching enemy (Flamethrower burning your own
            // Dwarf in the Box) is not damage to anything.
            if found.is_none() && retargeted {
                return;
            }
            hit.target = found.unwrap_or(UNNAMED_TARGET | (hit.target_max_hp as u32 & 0x0fff_ffff));
        }
        // Effect damage has no attacker; the server only sends it for your own damage.
        // Area records (box explosion, mines) name their owner instead: keep it. With no direct
        // hit yet to tell who you are, an owner while you've been casting is most likely you.
        if hit.effect && hit.owner != 0 && self.me().is_none() && self.last_cast.is_some_and(|c| t - c <= 60.0) {
            self.me_votes.push_back(hit.owner);
        }
        if hit.effect && hit.owner == 0 {
            match self.me() {
                Some(me) => {
                    // attacker stays 0: that's how source_name tells these from area records.
                    hit.owner = me;
                }
                None => {
                    // Only worth keeping if you've been casting: then it's most likely yours.
                    if self.last_cast.is_some_and(|c| t - c <= EFFECT_WINDOW) {
                        self.pending_effects.retain(|(pt, _)| t - pt <= EFFECT_WINDOW);
                        if self.pending_effects.len() < 64 {
                            self.pending_effects.push((t, hit));
                        }
                    }
                    return;
                }
            }
        }
        // Mobs hitting you, your party or each other.
        if self.is_mob(hit.owner) || self.is_mob(hit.attacker) || Some(hit.target) == self.me() || self.names.contains_key(&hit.target) {
            return;
        }
        // Overkill: the killing blow only counts the HP the target had left.
        // A target we haven't seen yet was at full HP (one-shots).
        if hit.target_hp == 0 && hit.target_max_hp > 0 {
            let left = match self.targets.get(&hit.target).filter(|ts| ts.max_hp == hit.target_max_hp) {
                Some(ts) => ts.hp,
                None => hit.target_max_hp,
            };
            hit.damage = hit.damage.min(left.min(u32::MAX as u64) as u32);
            if hit.damage == 0 {
                return; // already dead
            }
        }
        // Pets and summons are credited to their owner.
        let who = hit.owner;
        // A party member of another class can't be you (their hits land right after your casts too).
        let other_class = match (self.my_class, self.classes.get(&who).and_then(|c| c.0)) {
            (Some(mine), Some(theirs)) => mine != theirs,
            _ => false,
        };
        if let Some(c) = self.last_cast.filter(|_| !other_class) {
            if (0.0..=CAST_WINDOW).contains(&(t - c)) {
                self.me_votes.push_back(who);
                if self.me_votes.len() > ME_VOTES {
                    self.me_votes.pop_front();
                }
            }
        }
        self.hits_seen += 1;
        let (name, rank) = self.target_label(hit.target);
        let counted = self.counts(&rank);
        let me = self.me();

        // A real id for a target we were tracking under a made-up one: move its state over.
        if hit.target < UNNAMED_TARGET && hit.target_max_hp > 0 {
            let fake = UNNAMED_TARGET | (hit.target_max_hp as u32 & 0x0fff_ffff);
            if !self.targets.contains_key(&hit.target) {
                if let Some(ts) = self.targets.remove(&fake) {
                    self.targets.insert(hit.target, ts);
                }
            }
        }
        // Target HP is tracked for every hit, filtered or not.
        if hit.target_max_hp > 0 {
            let ts = self.targets.entry(hit.target).or_insert(TargetState {
                hp: hit.target_hp,
                max_hp: hit.target_max_hp,
                first_hit: t,
                last_hit: t,
                damage: 0,
                my_damage: 0,
            });
            if t >= ts.last_hit {
                ts.hp = hit.target_hp;
                ts.max_hp = hit.target_max_hp;
                ts.last_hit = t;
            }
            ts.damage += hit.damage as u64;
            if me == Some(who) {
                ts.my_damage += hit.damage as u64;
            }
        }
        self.targets.retain(|_, ts| t - ts.last_hit < TARGET_MEMORY);
        self.recent.push_back(HitView {
            time: t,
            damage: hit.damage,
            crit: hit.crit,
            attacker: hit.attacker,
            owner: hit.owner,
            is_me: me == Some(who),
            target: hit.target,
            target_name: name.clone(),
            target_rank: rank.clone(),
            counted,
        });
        if self.recent.len() > RECENT_HITS {
            self.recent.pop_front();
        }
        if !counted {
            return;
        }
        let new_fight = match &self.encounter {
            None => true,
            Some(e) => match &e.killed {
                Some((kt, _)) => t - kt > AFTER_KILL_GRACE,
                None => t - e.last > self.idle_timeout,
            },
        };
        if new_fight {
            self.finish();
            let mut attackers: HashMap<u32, AttackerStats> = HashMap::new();
            if let Some(me) = me {
                let pre = attackers.entry(me).or_default();
                for (ct, id) in &self.casts {
                    if t - ct <= PRE_FIGHT_CASTS && *ct <= t {
                        *pre.casts.entry(skill_name(*id)).or_default() += 1;
                    }
                }
            }
            for (id, q) in &self.party_casts {
                let pre = attackers.entry(*id).or_default();
                for (ct, skill) in q {
                    if t - ct <= PRE_FIGHT_CASTS && *ct <= t {
                        *pre.casts.entry(skill_name(*skill)).or_default() += 1;
                    }
                }
            }
            self.encounter = Some(Encounter { start: t, last: t, attackers, targets: HashMap::new(), killed: None, saved: false });
        }
        if me == Some(who) && hit.attacker != hit.owner {
            if let Some(m) = self.entities.get(&hit.attacker).and_then(|(tpl, _)| mercenary_name(tpl)) {
                self.my_mercenary = Some(m);
            }
        }
        let source = self.source_name(t, &hit, me == Some(who));
        // Direct hits: the element in the hit itself (masteries change a skill's element), else
        // the skill's own, else Physical. Effect records always carry their element.
        let element = hit
            .element
            .as_deref()
            .or_else(|| skill_element(&source))
            .or(if hit.effect { None } else { Some("Physical") });
        if me == Some(who) && !hit.effect && !hit.echo {
            if let Some(el) = element.filter(|e| *e != "Physical") {
                self.element_hits.insert(el.to_string(), (t, source.clone()));
            }
        }
        let enc = self.encounter.as_mut().unwrap();
        enc.last = enc.last.max(t);
        let a = enc.attackers.entry(who).or_default();
        a.total.add(hit.damage, hit.crit, element);
        a.sources.entry(source).or_default().add(hit.damage, hit.crit, element);
        let boss_down = hit.target_hp == 0 && matches!(rank.as_str(), "boss" | "worldboss");
        if boss_down && self.stop_on_boss_kill && enc.killed.is_none() {
            enc.killed = Some((t, name.clone()));
        }
        *enc.targets.entry((name, rank)).or_default() += hit.damage as u64;
    }

    fn finish(&mut self) {
        if let Some(e) = self.encounter.take() {
            if !e.saved && e.attackers.values().any(|a| a.total.hits > 0) {
                self.unsaved.push(self.view(&e, None));
            }
            self.history.push_front(e);
            self.history.truncate(HISTORY);
        }
    }

    /// Fights that ended since the last call (timed out, boss killed, reset or zone change), to save.
    /// A fight that is over but still shown stays on screen; it's only handed out once.
    pub fn take_finished(&mut self, now: f64) -> Vec<EncounterView> {
        let idle = self.idle_timeout;
        let over = self.encounter.as_ref().is_some_and(|e| {
            !e.saved
                && match &e.killed {
                    Some((kt, _)) => now - kt > AFTER_KILL_GRACE,
                    None => now - e.last > idle,
                }
        });
        if over {
            let view = self.view(self.encounter.as_ref().unwrap(), None);
            self.encounter.as_mut().unwrap().saved = true;
            if !view.attackers.is_empty() {
                self.unsaved.push(view);
            }
        }
        std::mem::take(&mut self.unsaved)
    }

    /// End the running fight now (app closing), so it gets saved.
    pub fn end_fight(&mut self) {
        self.finish();
    }

    pub fn reset(&mut self) {
        self.finish();
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn view(&self, e: &Encounter, now: Option<f64>) -> EncounterView {
        let over = e.killed.is_some() || now.is_none_or(|n| n - e.last > self.idle_timeout);
        // Live fights keep ticking between hits; finished ones end at the last hit.
        let duration = match now {
            Some(n) if !over => (n - e.start).max(1.0),
            _ => e.duration(),
        };
        let total: u64 = e.attackers.values().map(|a| a.total.damage).sum();
        let me = self.me();
        let mut attackers: Vec<AttackerView> = e
            .attackers
            .iter()
            .filter(|(_, a)| a.total.hits > 0)
            .map(|(id, a)| {
                let empty = Tally::default();
                let mut names: Vec<&String> = a.sources.keys().chain(a.casts.keys()).collect();
                names.sort();
                names.dedup();
                let mut sources: Vec<SourceView> = names
                    .into_iter()
                    .map(|name| {
                        let s = a.sources.get(name).unwrap_or(&empty);
                        SourceView {
                            name: name.clone(),
                            damage: s.damage,
                            dps: s.damage as f64 / duration,
                            hits: s.hits,
                            crits: s.crits,
                            crit_rate: s.crit_rate(),
                            max_hit: s.max_hit,
                            share: if a.total.damage > 0 { s.damage as f64 / a.total.damage as f64 } else { 0.0 },
                            casts: a.casts.get(name).copied().unwrap_or(0),
                            elements: s.element_views(),
                        }
                    })
                    .collect();
                sources.sort_by(|x, y| y.damage.cmp(&x.damage).then(y.casts.cmp(&x.casts)));
                let (class, level) = self.player_class(*id, me == Some(*id));
                AttackerView {
                    class,
                    level,
                    id: *id,
                    name: self.player_name(*id),
                    is_me: me == Some(*id),
                    damage: a.total.damage,
                    dps: a.total.damage as f64 / duration,
                    hits: a.total.hits,
                    crits: a.total.crits,
                    crit_rate: a.total.crit_rate(),
                    max_hit: a.total.max_hit,
                    share: if total > 0 { a.total.damage as f64 / total as f64 } else { 0.0 },
                    sources,
                    elements: a.total.element_views(),
                }
            })
            .collect();
        attackers.sort_by(|a, b| b.damage.cmp(&a.damage));
        let mut targets: Vec<TargetView> = e
            .targets
            .iter()
            .map(|((name, rank), dmg)| TargetView { name: name.clone(), rank: rank.clone(), damage: *dmg })
            .collect();
        targets.sort_by(|a, b| b.damage.cmp(&a.damage));
        EncounterView { start: e.start, duration, over, killed: e.killed.as_ref().map(|k| k.1.clone()), total, attackers, targets }
    }

    fn target_view(&self, id: u32, ts: &TargetState, now: f64) -> TargetHpView {
        let (name, rank) = self.target_label(id);
        let fight_time = (ts.last_hit - ts.first_hit).max(1.0);
        let dps = ts.damage as f64 / fight_time;
        let dead = ts.hp == 0;
        let since_last_hit = (now - ts.last_hit).max(0.0);
        TargetHpView {
            id,
            name,
            rank,
            hp: ts.hp,
            max_hp: ts.max_hp,
            pct: if ts.max_hp > 0 { ts.hp as f64 / ts.max_hp as f64 } else { 0.0 },
            dead,
            dps,
            my_damage: ts.my_damage,
            // Only estimate while the fight on it is active.
            time_to_kill: (!dead && dps > 0.0 && since_last_hit < self.idle_timeout).then(|| ts.hp as f64 / dps),
            fight_time,
            since_last_hit,
        }
    }

    fn tracked(&self, now: f64) -> Vec<TargetHpView> {
        let mut v: Vec<TargetHpView> =
            self.targets.iter().map(|(id, ts)| self.target_view(*id, ts, now)).collect();
        // Important first, then alive before dead, then biggest.
        v.sort_by(|a, b| {
            rank_priority(&b.rank)
                .cmp(&rank_priority(&a.rank))
                .then(a.dead.cmp(&b.dead))
                .then(b.max_hp.cmp(&a.max_hp))
                .then(a.since_last_hit.total_cmp(&b.since_last_hit))
        });
        v.truncate(MAX_TRACKED);
        v
    }

    pub fn snapshot(&self, now: f64) -> Snapshot {
        let tracked = self.tracked(now);
        // HP bar: the most important target, if it was hit within the fight timeout
        // (dead bosses stay up until then so you see the kill).
        let focus = tracked
            .iter()
            .find(|t| t.since_last_hit < self.idle_timeout && rank_priority(&t.rank) >= ELITE_PRIORITY)
            .or_else(|| tracked.iter().filter(|t| !t.dead && t.since_last_hit < self.idle_timeout).max_by_key(|t| t.max_hp))
            .cloned();
        Snapshot {
            me: self.me(),
            encounter: self.encounter.as_ref().map(|e| self.view(e, Some(now))),
            focus_target: focus,
            tracked_targets: tracked,
            recent_hits: self.recent.iter().rev().cloned().collect(),
            history: self.history.iter().map(|e| self.view(e, None)).collect(),
            known_entities: self.entities.len(),
        }
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}
