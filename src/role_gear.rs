//! Classic jungle items use normal equipment slots; roam is a boots blessing.
use mod_api_stable::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Roam {
    #[default]
    Auto,
    None,
    Conceal,
    Encourage,
    Favor,
    DireHit,
}
impl Roam {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::None,
            Self::None => Self::Conceal,
            Self::Conceal => Self::Encourage,
            Self::Encourage => Self::Favor,
            Self::Favor => Self::DireHit,
            Self::DireHit => Self::Auto,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::None => "None",
            Self::Conceal => "Conceal",
            Self::Encourage => "Encourage",
            Self::Favor => "Favor",
            Self::DireHit => "Dire Hit",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Choice {
    pub jungle: Jungle,
    pub jungle_first: bool,
    pub roam: Roam,
    pub boots_first: bool,
    pub roam_first: bool,
}
impl Default for Choice {
    fn default() -> Self {
        Self {
            jungle: Jungle::Auto,
            jungle_first: false,
            roam: Roam::Auto,
            boots_first: false,
            roam_first: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Jungle {
    #[default]
    Auto,
    None,
    RaptorMachete,
    BeastKiller,
    StarShard,
}
impl Jungle {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::None,
            Self::None => Self::RaptorMachete,
            Self::RaptorMachete => Self::BeastKiller,
            Self::BeastKiller => Self::StarShard,
            Self::StarShard => Self::Auto,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::None => "None",
            Self::RaptorMachete => "Raptor Machete",
            Self::BeastKiller => "Beast Killer",
            Self::StarShard => "Star Shard",
        }
    }
}
impl Choice {
    /// Shared routes cannot carry an owner's roaming selection.
    pub fn shared_default() -> Self {
        Self {
            roam: Roam::Auto,
            ..Default::default()
        }
    }
    pub fn disabled() -> Self {
        Self {
            jungle: Jungle::None,
            roam: Roam::None,
            ..Default::default()
        }
    }
}
pub(crate) const JUNGLE_KEYS: [&str; 7] = [
    "hunters_knife",
    "curse_sword",
    "nimble_blade",
    "pillager_axe",
    "star_shard",
    "raptor_machete",
    "beast_killer",
];
pub(crate) fn is_jungle_key(key: &str) -> bool {
    JUNGLE_KEYS.contains(&key.strip_prefix("mythical_").unwrap_or(key))
}
pub(crate) fn jungle_target(
    champion: &str,
    role: crate::build_config::Role,
    choice: Jungle,
) -> Option<&'static str> {
    use crate::build_config::Role;
    if role != Role::Jungle {
        return None;
    }
    match choice {
        Jungle::None => None,
        Jungle::RaptorMachete => Some("raptor_machete"),
        Jungle::BeastKiller => Some("beast_killer"),
        Jungle::StarShard => Some("star_shard"),
        Jungle::Auto => {
            let t = crate::champion_traits::traits(champion).unwrap_or_default();
            Some(if t.tank {
                "beast_killer"
            } else if t.scaling == Some(crate::champion_traits::Scaling::Ap) {
                "star_shard"
            } else {
                "raptor_machete"
            })
        }
    }
}
fn resolved_roam(name: &str, lane: Option<LaneV1>, roam: Roam) -> Roam {
    if lane != Some(LaneV1::Support) {
        return Roam::None;
    }
    if roam != Roam::Auto {
        return roam;
    }
    if lane != Some(LaneV1::Support) {
        return Roam::None;
    }
    let t = crate::champion_traits::traits(name).unwrap_or_default();
    if t.sustains && t.aids_allies() {
        Roam::Favor
    } else {
        Roam::Encourage
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Settings {
    pub thriving_gold: usize,
    pub thriving_interval_seconds: f64,
    pub assist_gold: usize,
    pub unlock_gold: usize,
    pub full_power_gold: usize,
    pub aura_range: usize,
    pub encourage_attack_min: i32,
    pub encourage_attack_max: i32,
    pub encourage_power_min: i32,
    pub encourage_power_max: i32,
    pub encourage_attack_speed: i32,
    pub favor_heal_min: usize,
    pub favor_heal_max: usize,
    pub favor_cooldown_seconds: f64,
    pub dire_hit_min_percent: usize,
    pub dire_hit_max_percent: usize,
    pub dire_hit_cooldown_seconds: f64,
    pub conceal_speed_min: i32,
    pub conceal_speed_max: i32,
    pub conceal_duration_seconds: f64,
    pub conceal_cooldown_seconds: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            thriving_gold: 3,
            thriving_interval_seconds: 5.,
            assist_gold: 10,
            unlock_gold: 0,
            full_power_gold: 480,
            aura_range: 50,
            encourage_attack_min: 5,
            encourage_attack_max: 12,
            encourage_power_min: 7,
            encourage_power_max: 18,
            encourage_attack_speed: 15,
            favor_heal_min: 240,
            favor_heal_max: 600,
            favor_cooldown_seconds: 15.,
            dire_hit_min_percent: 7,
            dire_hit_max_percent: 18,
            dire_hit_cooldown_seconds: 30.,
            conceal_speed_min: 30,
            conceal_speed_max: 75,
            conceal_duration_seconds: 5.,
            conceal_cooldown_seconds: 80.,
        }
    }
}
impl Settings {
    fn valid(&self) -> bool {
        self.thriving_gold <= 1000
            && self.assist_gold <= 1000
            && self.full_power_gold > 0
            && self.full_power_gold <= 100_000
            && (1..=200).contains(&self.aura_range)
            && [
                self.thriving_interval_seconds,
                self.favor_cooldown_seconds,
                self.dire_hit_cooldown_seconds,
                self.conceal_duration_seconds,
                self.conceal_cooldown_seconds,
            ]
            .iter()
            .all(|x| x.is_finite() && *x >= 0.1 && *x <= 600.)
            && self.favor_heal_min <= self.favor_heal_max
            && self.favor_heal_max <= 10_000
            && self.dire_hit_min_percent <= self.dire_hit_max_percent
            && self.dire_hit_max_percent <= 50
            && [
                (self.encourage_attack_min, self.encourage_attack_max),
                (self.encourage_power_min, self.encourage_power_max),
                (self.conceal_speed_min, self.conceal_speed_max),
            ]
            .iter()
            .all(|(a, b)| *a >= 0 && a <= b && *b <= 100)
            && (0..=100).contains(&self.encourage_attack_speed)
    }
}
fn settings() -> &'static Settings {
    static S: OnceLock<Settings> = OnceLock::new();
    S.get_or_init(|| {
        let path=crate::config::mod_dir().join("role-gear-config.json");
        match std::fs::read_to_string(path) {
            Ok(text)=>match serde_json::from_str::<Settings>(&text) {
                Ok(s) if s.valid()=>s,
                _=>{crate::hook::build_status("role-gear", "Invalid role-gear-config.json; safe defaults used. Restart after correcting it.".into()); Settings::default()}
            },
            Err(_)=>Settings::default(),
        }
    })
}

// Native spawn supplies match-side + seed and resolves per-player ownership.
#[derive(Clone)]
struct NativeSeat {
    seed: u64,
    side: usize,
    champion: String,
    role: crate::build_config::Role,
    choice: Choice,
}
static SEATS: Mutex<Vec<NativeSeat>> = Mutex::new(Vec::new());
pub(crate) fn seat(
    seed: u64,
    side: usize,
    champion: &str,
    role: crate::build_config::Role,
    choice: Choice,
) {
    if seed == 0 {
        return;
    }
    if let Ok(mut seats) = SEATS.lock() {
        seats.retain(|s| !(s.seed == seed && s.side == side && s.champion == champion));
        if seats.len() >= 640 {
            seats.drain(..320);
        }
        seats.push(NativeSeat {
            seed,
            side,
            champion: champion.into(),
            role,
            choice,
        });
    }
}
fn native_seat(seed: u64, side: usize, name: &str, lane: Option<LaneV1>) -> Option<NativeSeat> {
    let seats = SEATS.lock().ok()?;
    if let Some(exact) = seats
        .iter()
        .rev()
        .find(|s| s.seed == seed && s.side == side && s.champion == name)
    {
        return Some(exact.clone());
    }
    // Entity names may be display names. A unique exact match-side + lane
    // binds the native champion key without borrowing another holder's choice.
    let lane = lane?;
    let mut matching = seats
        .iter()
        .filter(|s| s.seed == seed && s.side == side && role_lane(s.role) == Some(lane));
    let seat = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(seat.clone())
}
/// Presentation-only lane fallback; never guess another champion's seat.
pub(crate) fn presentation_lane(sim: &StableSim<'_>, player: usize) -> Option<LaneV1> {
    let p = sim.get_player(player)?;
    p.lane().or_else(|| {
        let name = p.champion()?.name()?;
        let seat = native_seat(simulation_seed(sim), p.team(), &name, None)?;
        role_lane(seat.role)
    })
}
fn role_lane(role: crate::build_config::Role) -> Option<LaneV1> {
    use crate::build_config::Role;
    match role {
        Role::Top => Some(LaneV1::Top),
        Role::Jungle => Some(LaneV1::Jungle),
        Role::Mid => Some(LaneV1::Mid),
        Role::Bottom => Some(LaneV1::Bottom),
        Role::Support => Some(LaneV1::Support),
        _ => None,
    }
}
pub(crate) fn simulation_seed(sim: &StableSim<'_>) -> u64 {
    let seed = sim.seed();
    if seed != 0 {
        seed
    } else {
        crate::tactics::provider_seed(game_id(sim)).unwrap_or(0)
    }
}
pub(crate) fn career_started() {
    if let Ok(mut timeline) = ROAM_TIMELINE.lock() {
        *timeline = None;
    }
    if let Ok(mut s) = SEATS.lock() {
        s.clear();
    }
}

#[derive(Default)]
struct Carrier {
    player: usize,
    entity: usize,
    team: usize,
    choice: Choice,
    bound: bool,
    lane: Option<LaneV1>,
    has_jungle_item: bool,
    jungle_key: Option<String>,
    jungle_stacks: usize,
    boots: bool,
    earned: usize,
    assists: usize,
    kills: usize,
    favor_at: usize,
    dire_at: usize,
    conceal_at: usize,
    aid_cursor: usize,
}
#[derive(Clone, Copy)]
struct Hit {
    source: usize,
    target: usize,
    lethal: bool,
    dire_eligible: bool,
}
#[derive(Default)]
struct Game {
    seed: u64,
    tick: usize,
    carriers: HashMap<usize, Carrier>,
    tokens: HashMap<usize, usize>,
    monsters: HashSet<usize>,
    hits: Vec<Hit>,
    conceal: HashMap<usize, usize>,
    revealed: HashSet<usize>,
    dead_monsters: HashSet<usize>,
    max_hp: HashMap<usize, usize>,
    conceal_ready: HashMap<usize, usize>,
    last_thriving: Option<usize>,
    processed_tick: Option<usize>,
}
thread_local! { static GAMES: RefCell<HashMap<usize,Game>> = RefCell::new(HashMap::new()); static PROC:Cell<bool> = const {Cell::new(false)}; }
fn game_id(sim: &StableSim<'_>) -> usize {
    crate::heal_shield_power::game_of(sim)
}
fn interpolate(gold: usize, lo: usize, hi: usize, cfg: &Settings) -> usize {
    // Blessings work immediately on completed boots. Thriving only grows
    // their strength, smoothly from zero gold; it never unlocks the effect.
    lo + (hi - lo) * gold.min(cfg.full_power_gold) / cfg.full_power_gold.max(1)
}
fn boot(keys: &[String]) -> bool {
    keys.iter().any(|k| {
        matches!(
            k.as_str(),
            "arcane_boots"
                | "demon_shoes"
                | "magic_shoes"
                | "rapid_boots"
                | "swift_boots"
                | "tough_boots"
                | "warrior_boots"
        )
    })
}
pub(crate) fn start(sim: &StableSim<'_>) {
    let mut game = Game {
        seed: simulation_seed(sim),
        ..Default::default()
    };
    bind_carriers(sim, &mut game);
    prime(sim, &mut game);
    GAMES.with(|g| {
        let mut g = g.borrow_mut();
        if g.len() >= 16 {
            g.clear();
        }
        g.insert(game_id(sim), game);
    });
}
fn bind_carriers(sim: &StableSim<'_>, game: &mut Game) {
    // Match-start may precede champion spawn and the native ownership snapshot.
    // Bind late arrivals without resetting income or cooldowns already earned.
    for i in 0..sim.player_count() {
        let Some(p) = sim.player_at(i) else {
            continue;
        };
        let Some(e) = p.champion() else {
            continue;
        };
        let c = game.carriers.entry(e.id()).or_insert_with(|| Carrier {
            player: p.id(),
            entity: e.id(),
            team: p.team(),
            choice: Choice::disabled(),
            assists: p.assists(),
            kills: p.kills(),
            ..Default::default()
        });
        c.lane = p.lane().or(c.lane);
        if !c.bound || sim.tick() % 15 == 0 {
            let name = e.name().unwrap_or_default();
            if let Some(s) = native_seat(game.seed, p.team(), &name, p.lane()) {
                c.lane = p.lane().or_else(|| role_lane(s.role));
                c.choice = s.choice;
                c.choice.roam = resolved_roam(&s.champion, c.lane, s.choice.roam);
                c.bound = true;
            }
        }
    }
}
/// ABI-compatible fallback when an older host does not drive match hooks.
/// Items may update many times per tick; only the first fallback does work.
pub(crate) fn item_tick(sim: &mut StableSim<'_>) {
    // Establish native hook context even without Flask/heal-power carriers.
    // Reassert on every item call, including a return to this sim on a thread.
    crate::heal_shield_power::on_match_tick(sim);
    static RECEIVED: OnceLock<()> = OnceLock::new();
    if RECEIVED.set(()).is_ok() {
        crate::hook::build_status(
            "roaming/callbacks",
            format!(
                "item update received; players={} origin={:?}",
                sim.player_count(),
                sim.sim_origin().map(|o| o.kind)
            ),
        );
    }
    let done = GAMES.with(|g| {
        g.borrow()
            .get(&game_id(sim))
            .is_some_and(|g| g.processed_tick == Some(sim.tick()))
    });
    if !done {
        crate::mlbb::rules::on_match_tick(sim);
        tick(sim);
    }
}

// Native landing: source token is translated using verified entity tokens.
// Own bonus procs do not recursively trigger role equipment.
pub(crate) fn landing(
    game: usize,
    target: usize,
    source: Option<usize>,
    amount: usize,
    hp: usize,
    shield: usize,
) -> usize {
    if amount == 0 || PROC.with(Cell::get) {
        return amount;
    }
    GAMES.with(|all| {
        let Ok(mut all) = all.try_borrow_mut() else {
            return amount;
        };
        let Some(g) = all.get_mut(&game) else {
            return amount;
        };
        let source = source.and_then(|t| g.tokens.get(&t).copied());
        g.revealed.insert(target);
        let Some(source) = source else { return amount };
        g.revealed.insert(source);
        let mut result = amount;
        if let Some(c) = g.carriers.get(&source).filter(|c| c.has_jungle_item) {
            let key = c.jungle_key.as_deref().unwrap_or("hunters_knife");
            let base = key.strip_prefix("mythical_").unwrap_or(key);
            if matches!(base, "curse_sword" | "star_shard") {
                result = result.saturating_add(
                    result
                        .saturating_mul(c.jungle_stacks)
                        .saturating_mul(if key.starts_with("mythical_") { 75 } else { 60 })
                        / 10_000,
                );
            }
            if g.monsters.contains(&target) {
                let percent = if base == "hunters_knife" {
                    30
                } else if matches!(base, "curse_sword" | "nimble_blade" | "pillager_axe") {
                    40
                } else {
                    50
                };
                result = result.saturating_add(result.saturating_mul(percent) / 100);
            }
        }
        let amount = result;
        if g.hits.len() < 512 {
            g.hits.push(Hit {
                source,
                target,
                lethal: amount >= hp.saturating_add(shield),
                dire_eligible: g.max_hp.get(&target).is_some_and(|max| {
                    hp.saturating_sub(amount.saturating_sub(shield)) * 100 <= *max * 35
                }),
            });
        }
        amount
    })
}

fn bonus(sim: &mut StableSim<'_>, source: usize, target: usize, amount: usize) {
    struct Guard(bool);
    impl Drop for Guard {
        fn drop(&mut self) {
            PROC.with(|p| p.set(self.0));
        }
    }
    let _guard = Guard(PROC.with(|p| p.replace(true)));
    sim.deal_damage_typed(
        source,
        target,
        amount / 2,
        DamageTypeV1::Ad,
        AttackTypeV1::Item,
    );
    sim.deal_damage_typed(
        source,
        target,
        amount - amount / 2,
        DamageTypeV1::Ap,
        AttackTypeV1::Item,
    );
}

pub(crate) fn tick(sim: &mut StableSim<'_>) {
    let key = game_id(sim);
    if !GAMES.with(|a| a.borrow().contains_key(&key)) {
        start(sim);
    }
    let Some(mut g) = GAMES.with(|a| a.borrow_mut().remove(&key)) else {
        return;
    };
    if g.seed != simulation_seed(sim) {
        start(sim);
        return;
    }
    bind_carriers(sim, &mut g);
    let cfg = settings();
    let now = sim.tick();
    g.tick = now;
    g.processed_tick = Some(now);
    let mut ids: Vec<_> = g.carriers.keys().copied().collect();
    ids.sort_unstable();
    for &id in &ids {
        let c = g.carriers.get_mut(&id).unwrap();
        let Some(p) = sim.get_player(c.player) else {
            continue;
        };
        c.boots = boot(&p.item_keys());
        let delta_assists = p.assists().saturating_sub(c.assists);
        let delta_kills = p.kills().saturating_sub(c.kills);
        c.assists = p.assists();
        c.kills = p.kills();
        let keys = p.item_keys();
        c.jungle_key = if p.lane().or(c.lane) == Some(LaneV1::Jungle) {
            crate::mlbb::rules::selected(&keys, "jungle").map(str::to_owned)
        } else {
            None
        };
        c.has_jungle_item = c.jungle_key.is_some();
        if !c.has_jungle_item {
            c.jungle_stacks = 0;
        }
        let max = p
            .champion()
            .filter(|e| e.is_alive())
            .map_or(0, |e| e.hp().1);
        if c.has_jungle_item && delta_kills + delta_assists > 0 {
            sim.player_add_gold(c.player, ((delta_kills + delta_assists) * 10) as i64);
            if c.jungle_key
                .as_deref()
                .is_some_and(|k| k.ends_with("star_shard"))
                && delta_kills > 0
            {
                let percent = if c.jungle_key.as_deref().unwrap().starts_with("mythical_") {
                    125
                } else {
                    100
                };
                crate::heal_shield_power::unraised(|| {
                    sim.heal(id, id, max * percent * delta_kills / 1000)
                });
            }
        }
        if c.boots && c.choice.roam != Roam::None && delta_assists > 0 {
            let gold = delta_assists * cfg.assist_gold;
            sim.player_add_gold(c.player, gold as i64);
            c.earned = c.earned.saturating_add(gold).min(cfg.full_power_gold);
        }
    }
    if now > 0
        && g.last_thriving != Some(now)
        && now % crate::ticks(cfg.thriving_interval_seconds).max(1) == 0
    {
        g.last_thriving = Some(now);
        // One Thriving recipient per team; wallet gold is an explicit TFM2 substitution.
        let mut winners: HashMap<usize, (usize, usize)> = HashMap::new();
        for &id in &ids {
            let c = &g.carriers[&id];
            if !c.boots || c.choice.roam == Roam::None {
                continue;
            }
            let gold = sim.get_player(c.player).map_or(usize::MAX, |p| p.gold());
            let entry = winners.entry(c.team).or_insert((gold, id));
            if (gold, id) < *entry {
                *entry = (gold, id);
            }
        }
        let mut winners: Vec<_> = winners.into_values().map(|(_, id)| id).collect();
        winners.sort_unstable();
        for id in winners {
            let c = g.carriers.get_mut(&id).unwrap();
            sim.player_add_gold(c.player, cfg.thriving_gold as i64);
            c.earned = c
                .earned
                .saturating_add(cfg.thriving_gold)
                .min(cfg.full_power_gold);
        }
    }
    let hits = std::mem::take(&mut g.hits);
    for hit in hits {
        if hit.lethal && g.monsters.contains(&hit.target) && g.dead_monsters.insert(hit.target) {
            if let Some(c) = g
                .carriers
                .get_mut(&hit.source)
                .filter(|c| c.has_jungle_item)
            {
                let key = c.jungle_key.as_deref().unwrap_or("hunters_knife");
                let base = key.strip_prefix("mythical_").unwrap_or(key);
                let myth = key.starts_with("mythical_");
                sim.player_add_gold(c.player, if base == "hunters_knife" { 5 } else { 8 });
                if base != "hunters_knife" {
                    let max = sim
                        .get_entity(c.entity)
                        .filter(|e| e.is_alive())
                        .map_or(0, |e| e.hp().1);
                    crate::heal_shield_power::unraised(|| {
                        sim.heal(c.entity, c.entity, max * if myth { 5 } else { 4 } / 100)
                    });
                    c.jungle_stacks = (c.jungle_stacks + 1).min(
                        if matches!(base, "curse_sword" | "nimble_blade" | "pillager_axe") {
                            10
                        } else {
                            15
                        },
                    );
                }
            }
        }
        let Some(c) = g.carriers.get_mut(&hit.source) else {
            continue;
        };
        if c.choice.roam != Roam::DireHit
            || !c.boots
            || now < c.dire_at
        {
            continue;
        }
        let Some((_hp, max)) = sim
            .get_entity(hit.target)
            .filter(|e| e.is_alive() && e.is_champion() && e.team() != c.team)
            .map(|e| e.hp())
        else {
            continue;
        };
        if hit.dire_eligible {
            c.dire_at = now + crate::ticks(cfg.dire_hit_cooldown_seconds);
            bonus(
                sim,
                c.entity,
                hit.target,
                max * interpolate(
                    c.earned,
                    cfg.dire_hit_min_percent,
                    cfg.dire_hit_max_percent,
                    cfg,
                ) / 100,
            );
        }
    }
    for &id in &ids {
        let c = &g.carriers[&id];
        let key = c.jungle_key.as_deref().unwrap_or("");
        let base = key.strip_prefix("mythical_").unwrap_or(key);
        let scale = if key.starts_with("mythical_") {
            1.25
        } else {
            1.
        };
        let ad = if matches!(base, "nimble_blade" | "raptor_machete") {
            (c.jungle_stacks as f64 * 1.4 * scale).round() as i32
        } else {
            0
        };
        let def = if matches!(base, "pillager_axe" | "beast_killer") {
            (c.jungle_stacks as f64 * 2.1 * scale).round() as i32
        } else {
            0
        };
        if ad == 0 && def == 0 {
            continue;
        }
        crate::refresh_buff(
            sim,
            id,
            "mlbb_jungle_gorge",
            &BuffV1 {
                attack: ad,
                defence: def,
                magic_resistance: def,
                ..BuffV1::timed("mlbb_jungle_gorge", 2)
            },
        );
    }
    let mut encourages: HashMap<usize, (i32, i32)> = HashMap::new();
    for &id in &ids {
        let c = g.carriers.get_mut(&id).unwrap();
        let (cursor, aids) = crate::mlbb::rules::aid_events(sim, id, c.aid_cursor);
        c.aid_cursor = cursor;
        if !c.boots
            || !sim.get_entity(id).is_some_and(|e| e.is_alive())
        {
            continue;
        }
        let allies: Vec<_> = ids
            .iter()
            .copied()
            .filter(|a| {
                sim.get_entity(*a)
                    .is_some_and(|e| e.is_alive() && e.team() == c.team)
                    && sim.distance_sq(id, *a)
                        <= ((cfg.aura_range * crate::DISTANCE_UNITS_PER_RANGE) as u64).pow(2)
            })
            .collect();
        match c.choice.roam {
            Roam::Encourage => {
                let ad = interpolate(
                    c.earned,
                    cfg.encourage_attack_min as usize,
                    cfg.encourage_attack_max as usize,
                    cfg,
                ) as i32;
                let ap = interpolate(
                    c.earned,
                    cfg.encourage_power_min as usize,
                    cfg.encourage_power_max as usize,
                    cfg,
                ) as i32;
                for a in allies {
                    let e = encourages.entry(a).or_default();
                    e.0 = e.0.max(ad);
                    e.1 = e.1.max(ap);
                }
            }
            Roam::Favor
                if now >= c.favor_at
                    && aids.iter().any(|(target, _)| {
                        sim.get_entity(*target).is_some_and(|e| e.team() == c.team)
                    }) =>
            {
                if let Some(target) = allies
                    .into_iter()
                    .filter_map(|a| sim.get_entity(a).map(|e| (a, e.hp())))
                    .filter(|(_, (hp, max))| hp < max)
                    .min_by_key(|(a, (hp, _))| (*hp, *a))
                    .map(|x| x.0)
                {
                    c.favor_at = now + crate::ticks(cfg.favor_cooldown_seconds);
                    sim.heal(
                        id,
                        target,
                        interpolate(c.earned, cfg.favor_heal_min, cfg.favor_heal_max, cfg),
                    );
                }
            }
            Roam::Conceal if now >= c.conceal_at && allies.len() >= 2 => {
                let allies: Vec<_> = allies
                    .into_iter()
                    .filter(|a| {
                        !g.revealed.contains(a)
                            && now >= g.conceal_ready.get(a).copied().unwrap_or(0)
                    })
                    .collect();
                if allies.len() < 2 {
                    continue;
                }
                let enemy_close = ids.iter().any(|a| {
                    sim.get_entity(*a)
                        .is_some_and(|e| e.is_alive() && e.team() != c.team)
                        && sim.distance_sq(id, *a)
                            <= ((cfg.aura_range * 2 * crate::DISTANCE_UNITS_PER_RANGE) as u64)
                                .pow(2)
                });
                if enemy_close {
                    c.conceal_at = now + crate::ticks(cfg.conceal_cooldown_seconds);
                    for a in allies {
                        if now >= g.conceal_ready.get(&a).copied().unwrap_or(0) {
                            g.conceal_ready
                                .insert(a, now + crate::ticks(cfg.conceal_cooldown_seconds));
                            g.conceal
                                .insert(a, now + crate::ticks(cfg.conceal_duration_seconds));
                            let speed = interpolate(
                                c.earned,
                                cfg.conceal_speed_min as usize,
                                cfg.conceal_speed_max as usize,
                                cfg,
                            ) as i32;
                            crate::refresh_buff(
                                sim,
                                a,
                                "mlbb_roam_conceal",
                                &BuffV1 {
                                    move_speed_mult: speed,
                                    ..BuffV1::timed(
                                        "mlbb_roam_conceal",
                                        crate::ticks(cfg.conceal_duration_seconds),
                                    )
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut buffs: Vec<_> = encourages.into_iter().collect();
    buffs.sort_unstable_by_key(|x| x.0);
    for (id, (ad, ap)) in buffs {
        crate::refresh_buff(
            sim,
            id,
            "mlbb_roam_encourage",
            &BuffV1 {
                attack: ad,
                magic_power: ap,
                attack_speed_mult: cfg.encourage_attack_speed,
                ..BuffV1::timed("mlbb_roam_encourage", 2)
            },
        );
    }
    g.conceal.retain(|id, until| {
        if now >= *until
            || g.revealed.contains(id)
            || !sim.get_entity(*id).is_some_and(|e| e.is_alive())
        {
            sim.entity_remove_buff(*id, "mlbb_roam_conceal");
            false
        } else {
            sim.entity_set_invisible(*id, 2);
            true
        }
    });
    g.revealed.clear();
    publish_roam_status(sim, &g, cfg);
    prime(sim, &mut g);
    GAMES.with(|a| a.borrow_mut().insert(key, g));
}
fn prime(sim: &StableSim<'_>, g: &mut Game) {
    let key = game_id(sim);
    g.tokens.clear();
    g.monsters.clear();
    g.max_hp.clear();
    let ids: Vec<_> = g.carriers.keys().copied().collect();
    for i in 0..sim.entity_count() {
        let Some(e) = sim.entity_at(i) else { continue };
        let id = e.id();
        g.max_hp.insert(id, e.hp().1);
        if e.is_alive() {
            g.dead_monsters.remove(&id);
        }
        if !e.is_champion() && !e.is_minion() && !e.is_tower() {
            g.monsters.insert(id);
        }
    }
    for &id in &ids {
        if let Some(token) = crate::tactics::heal_shield::entity_token(key, id) {
            g.tokens.insert(token, id);
        }
    }
}

#[cfg(test)]
#[path = "role_gear_tests.rs"]
mod tests;

pub(crate) fn prioritize_boots(slots: &mut [Option<String>]) -> bool {
    let Some(index) = slots.iter().position(|s| {
        s.as_deref().is_some_and(|k| {
            matches!(
                k,
                "arcane_boots"
                    | "demon_shoes"
                    | "magic_shoes"
                    | "rapid_boots"
                    | "swift_boots"
                    | "tough_boots"
                    | "warrior_boots"
            )
        })
    }) else {
        return false;
    };
    slots[..=index].rotate_right(1);
    true
}
pub(crate) fn init() {
    use std::io::Write;
    let s = settings();
    let path = crate::config::mod_dir().join("role-gear-config.json");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        if let Ok(json) = serde_json::to_string_pretty(s) {
            let _ = writeln!(f, "{json}");
        }
    }
}

pub(crate) fn apply_priorities(
    slots: &mut [Option<String>],
    choice: Choice,
    champion: Option<&str>,
    role: crate::build_config::Role,
) {
    let choice=crate::first_slot_rules::apply(choice,role);
    if choice.jungle_first && !slots.iter().flatten().any(|k| is_jungle_key(k)) {
        if let (Some(empty), Some(champ)) = (slots.iter().position(Option::is_none), champion) {
            if let Some(key) =
                jungle_target(champ, role, choice.jungle)
            {
                slots[empty] = Some(key.into());
            }
        }
    }
    let first = if wants_roam_boots(role, choice) {
        choice.roam_first
    } else {
        choice.boots_first
    };
    if first {
        prioritize_boots(slots);
    }
    if choice.jungle_first {
        if let Some(i) = slots
            .iter()
            .position(|s| s.as_deref().is_some_and(is_jungle_key))
        {
            slots[..=i].rotate_right(1);
        }
    }
}

/// Uses an existing unpinned slot; never grows the six-item inventory.
pub(crate) fn plan_jungle(
    champion: &str,
    role: crate::build_config::Role,
    choice: Choice,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    resolve: impl Fn(&str) -> Option<usize>,
    key: impl Fn(usize) -> Option<String>,
) {
    let Some(target) = jungle_target(champion, role, choice.jungle) else {
        return;
    };
    if build.iter().enumerate().any(|(slot, i)| {
        pinned.get(slot).copied().unwrap_or(false) && key(*i).is_some_and(|k| is_jungle_key(&k))
    }) || reserved
        .iter()
        .any(|i| key(*i).is_some_and(|k| is_jungle_key(&k)))
    {
        return;
    }
    let Some(index) = resolve(&format!("mythical_{target}")).or_else(|| resolve(target)) else {
        return;
    };
    if let Some(slot) = (0..build.len())
        .find(|i| {
            !pinned.get(*i).copied().unwrap_or(false)
                && key(build[*i]).is_some_and(|k| is_jungle_key(&k))
        })
        .or_else(|| {
            (0..build.len()).find(|i| {
                !pinned.get(*i).copied().unwrap_or(false)
                    && key(build[*i]).is_none_or(|k| !crate::smart_builds::is_boots(&k))
            })
        })
    {
        build[slot] = index;
    }
}

pub(crate) fn jungle_item_active(sim: &StableSim<'_>, entity: usize, key: &str) -> bool {
    (0..sim.player_count())
        .filter_map(|i| sim.player_at(i))
        .find(|p| p.champion().is_some_and(|e| e.id() == entity))
        .is_some_and(|p| {
            p.lane() == Some(LaneV1::Jungle)
                && crate::mlbb::rules::selected(&p.item_keys(), "jungle") == Some(key)
        })
}

/// The engine plans four slots; the existing six-slot hook fills the last two.
/// A role choice may use a blank late slot, but must not duplicate a reserved pin.
pub(crate) fn extra_jungle_target(
    champion: &str,
    role: crate::build_config::Role,
    choice: Choice,
    spoken: &[String],
) -> Option<&'static str> {
    if spoken.iter().any(|k| is_jungle_key(k)) {
        return None;
    }
    jungle_target(champion, role, choice.jungle)
}

/// A selected blessing needs actual completed boots even in No Boots mode.
/// Auto applies only to the Support role; None never forces a pair.
pub(crate) fn wants_roam_boots(role: crate::build_config::Role, choice: Choice) -> bool {
    role == crate::build_config::Role::Support && choice.roam != Roam::None
}

/// Keep normal boots in a free position, respecting pins and jungle equipment.
pub(crate) fn plan_roam_boots(
    role: crate::build_config::Role,
    choice: Choice,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    boots: Option<usize>,
    key: impl Fn(usize) -> Option<String>,
) {
    if !wants_roam_boots(role, choice) {
        return;
    }
    if reserved
        .iter()
        .any(|i| key(*i).is_some_and(|k| crate::smart_builds::is_boots(&k)))
    {
        return;
    }
    let held = build
        .iter()
        .position(|i| key(*i).is_some_and(|k| crate::smart_builds::is_boots(&k)));
    let movable = |slot: usize| {
        !pinned.get(slot).copied().unwrap_or(false)
            && !key(build[slot]).is_some_and(|k| is_jungle_key(&k))
    };
    if let Some(here) = held {
        if choice.roam_first && !pinned.get(here).copied().unwrap_or(false) {
            if let Some(at) = (0..here).find(|i| {
                !pinned.get(*i).copied().unwrap_or(false)
                    && (!choice.jungle_first || !key(build[*i]).is_some_and(|k| is_jungle_key(&k)))
            }) {
                build.swap(at, here);
            }
        }
        return;
    }
    let Some(boots) = boots else {
        return;
    };
    // Default second item, or first free item when Boots First is enabled.
    let at = if choice.roam_first {
        (0..build.len()).find(|i| movable(*i))
    } else {
        (1..build.len())
            .chain(0..build.len().min(1))
            .find(|i| movable(*i))
    };
    if let Some(at) = at {
        build[at] = boots;
        if choice.roam_first {
            if let Some(first) = (0..at).find(|i| {
                !pinned.get(*i).copied().unwrap_or(false)
                    && (!choice.jungle_first || !key(build[*i]).is_some_and(|k| is_jungle_key(&k)))
            }) {
                build.swap(first, at);
            }
        }
    }
}

pub(crate) fn plan_role_gear(
    champion: &str,
    role: crate::build_config::Role,
    choice: Choice,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    resolve: impl Fn(&str) -> Option<usize>,
    key: impl Fn(usize) -> Option<String>,
) {
    plan_jungle(
        champion, role, choice, build, pinned, reserved, &resolve, &key,
    );
    let boots = crate::build_config::rule_boots(champion)
        .unwrap_or_else(|| crate::smart_builds::boots_for(champion, role, &[]).into());
    plan_roam_boots(role, choice, build, pinned, reserved, resolve(&boots), &key);
    // Global first-slot rules are final ordering constraints, after normal AI
    // boots placement. The effective pin row uses the same ordering.
    if choice.jungle_first && role == crate::build_config::Role::Jungle {
        if let Some(at)=build.iter().position(|i|key(*i).is_some_and(|k|is_jungle_key(&k))) {
            build[..=at].rotate_right(1);
        }
    } else if choice.roam_first && wants_roam_boots(role,choice) {
        if let Some(at)=build.iter().position(|i|key(*i).is_some_and(|k|is_completed_boot(&k))) {
            build[..=at].rotate_right(1);
        }
    }
}

// UI-only snapshots follow the watched simulation, never background fixtures.
// Keep history because simulation can run ahead of the displayed match clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoamStatus {
    pub roam: Roam,
    pub progress_known: bool,
    pub boots: bool,
    pub boot_slots: Vec<(usize, String)>,
    pub alive: bool,
    pub earned: usize,
    pub unlock: usize,
    pub full_power: usize,
    pub ready_at: usize,
    pub concealed_until: usize,
}
impl RoamStatus {
    pub fn display(&self, now: usize) -> (String, &'static str) {
        if self.roam == Roam::None {
            return ("Roam: None\nOff".into(), "#aeb4c3ff");
        }
        if self.roam == Roam::Auto {
            return ("Roam: Auto\nWaiting for role".into(), "#aeb4c3ff");
        }
        if !self.progress_known {
            return (
                format!("{}\nProgress unavailable", self.roam.label()),
                "#aeb4c3ff",
            );
        }
        let goal = self.full_power;
        let title = format!("{} · Growth {}/{}", self.roam.label(), self.earned, goal);
        let (state, color) = if !self.boots {
            ("Needs boots".into(), "#d6be7aff")
        } else if !self.alive {
            ("Dead".into(), "#aeb4c3ff")

        } else if self.concealed_until > now {
            (
                format!("Active {}s", remaining_seconds(self.concealed_until, now)),
                "#92dc7bff",
            )
        } else if self.ready_at > now {
            (
                format!("Cooldown {}s", remaining_seconds(self.ready_at, now)),
                "#84b9ffff",
            )
        } else if self.roam == Roam::Encourage {
            ("Aura active".into(), "#92dc7bff")
        } else {
            ("Ready".into(), "#92dc7bff")
        };
        (format!("{title}\n{state}"), color)
    }
}
fn remaining_seconds(until: usize, now: usize) -> usize {
    (until.saturating_sub(now) as f64 / crate::TICKS_PER_SECOND).ceil() as usize
}
#[derive(Default)]
struct RoamTimeline {
    seed: u64,
    replay: u64,
    tick: usize,
    seats: [[Vec<(usize, RoamStatus)>; 5]; 2],
}
impl RoamTimeline {
    fn record(
        &mut self,
        seed: u64,
        replay: u64,
        tick: usize,
        rows: Vec<(usize, usize, RoamStatus)>,
    ) {
        if self.seed != seed || self.replay != replay {
            *self = Self {
                seed,
                replay,
                ..Default::default()
            };
        }
        if tick < self.tick {
            for seat in self.seats.iter_mut().flatten() {
                seat.retain(|(at, _)| *at < tick);
            }
        }
        self.tick = tick;
        for (side, lane, value) in rows {
            let Some(seat) = self.seats.get_mut(side).and_then(|s| s.get_mut(lane)) else {
                continue;
            };
            if seat.last().is_some_and(|(at, _)| *at == tick) {
                seat.pop();
            }
            if seat.last().is_none_or(|(_, old)| old != &value) {
                seat.push((tick, value));
            }
        }
    }
    fn at(&self, seed: u64, side: usize, lane: usize, tick: usize) -> Option<RoamStatus> {
        if self.seed != seed || tick > self.tick.saturating_add(crate::ticks(1.)) {
            return None;
        }
        self.seats
            .get(side)?
            .get(lane)?
            .iter()
            .rev()
            .find(|(at, _)| *at <= tick)
            .map(|(_, s)| s.clone())
    }
}
static ROAM_TIMELINE: Mutex<Option<RoamTimeline>> = Mutex::new(None);
fn publish_roam_status(sim: &StableSim<'_>, g: &Game, cfg: &Settings) {
    if sim.tick() % 15 != 0 {
        return;
    }
    let origin = sim.sim_origin();
    let native = crate::tactics::rendered_game(game_id(sim));
    if !visible_origin(origin.map(|o| o.kind), native) {
        return;
    }
    crate::hook::build_status(
        "roaming/telemetry",
        format!(
            "origin={:?} native_rendered={native} carriers={} bound={}",
            origin.map(|o| o.kind),
            g.carriers.len(),
            g.carriers.values().filter(|c| c.bound).count()
        ),
    );
    let rows = g
        .carriers
        .values()
        .filter_map(|c| {
            let p = sim.get_player(c.player)?;
            let lane = p.lane().or(c.lane)?.code() as usize;
            Some((
                c.team,
                lane,
                RoamStatus {
                    roam: c.choice.roam,
                    progress_known: true,
                    boots: c.boots,
                    boot_slots: p
                        .item_keys()
                        .into_iter()
                        .enumerate()
                        .filter(|(_, k)| is_completed_boot(k))
                        .collect(),
                    alive: p.champion().is_some_and(|e| e.is_alive()),
                    earned: c.earned,
                    unlock: 0,
                    full_power: cfg.full_power_gold,
                    ready_at: match c.choice.roam {
                        Roam::Favor => c.favor_at,
                        Roam::DireHit => c.dire_at,
                        Roam::Conceal => c.conceal_at,
                        _ => 0,
                    },
                    concealed_until: if c.choice.roam == Roam::Conceal {
                        g.conceal.get(&c.entity).copied().unwrap_or(0)
                    } else {
                        0
                    },
                },
            ))
        })
        .collect();
    if let Ok(mut ledger) = ROAM_TIMELINE.lock() {
        ledger.get_or_insert_with(Default::default).record(
            g.seed,
            origin.map_or(0, |o| o.replay_id),
            sim.tick(),
            rows,
        );
    }
}
pub(crate) fn shown_roam_status(
    seed: Option<u64>,
    side: usize,
    lane: usize,
    tick: Option<usize>,
) -> Option<RoamStatus> {
    let seed = seed?;
    if let Some(status) =
        tick.and_then(|t| ROAM_TIMELINE.lock().ok()?.as_ref()?.at(seed, side, lane, t))
    {
        return Some(status);
    }
    let seats = SEATS.lock().ok()?;
    let mut matching = seats.iter().filter(|s| {
        s.seed == seed
            && s.side == side
            && role_lane(s.role).is_some_and(|l| l.code() as usize == lane)
    });
    let seat = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(RoamStatus {
        roam: resolved_for_role(&seat.champion, seat.role, seat.choice.roam),
        progress_known: false,
        boots: false,
        boot_slots: vec![],
        alive: false,
        earned: 0,
        unlock: 0,
        full_power: settings().full_power_gold,
        ready_at: 0,
        concealed_until: 0,
    })
}
pub(crate) fn visible_origin(kind: Option<u32>, native: bool) -> bool {
    match kind.and_then(SimOriginKindV1::from_code) {
        Some(
            SimOriginKindV1::ClientMatchView
            | SimOriginKindV1::ClientSpectate
            | SimOriginKindV1::ClientReplay,
        ) => true,
        None | Some(SimOriginKindV1::Unknown) => native,
        _ => false,
    }
}

pub(crate) fn is_completed_boot(key: &str) -> bool {
    matches!(
        key,
        "arcane_boots"
            | "demon_shoes"
            | "magic_shoes"
            | "rapid_boots"
            | "swift_boots"
            | "tough_boots"
            | "warrior_boots"
    )
}
pub(crate) fn resolved_for_role(name: &str, role: crate::build_config::Role, roam: Roam) -> Roam {
    resolved_roam(
        name,
        if role == crate::build_config::Role::Support {
            Some(LaneV1::Support)
        } else {
            None
        },
        roam,
    )
}
pub(crate) fn badge_colour(roam: Roam) -> Option<&'static str> {
    match roam {
        Roam::Conceal => Some("#aa75ffff"),
        Roam::Favor => Some("#65db8fff"),
        Roam::Encourage => Some("#ed5c6bff"),
        Roam::DireHit => Some("#65baffff"),
        _ => None,
    }
}
pub(crate) fn selected_roam_description(status: &RoamStatus, now: usize) -> String {
    let cfg = settings();
    let effect=match status.roam {
        Roam::Favor=>format!("Healing or shielding another ally restores {}–{} HP to the nearby injured ally with the lowest HP. Cooldown: {}s.",cfg.favor_heal_min,cfg.favor_heal_max,cfg.favor_cooldown_seconds),
        Roam::Encourage=>format!("Nearby allies gain {}–{} Attack, {}–{} Magic Power and {}% Attack Speed. Does not stack with another Encourage aura.",cfg.encourage_attack_min,cfg.encourage_attack_max,cfg.encourage_power_min,cfg.encourage_power_max,cfg.encourage_attack_speed),
        Roam::Conceal=>format!("Automatically conceals nearby allies and grants {}–{}% Movement Speed for up to {}s. Damage ends the effect. Cooldown: {}s.",cfg.conceal_speed_min,cfg.conceal_speed_max,cfg.conceal_duration_seconds,cfg.conceal_cooldown_seconds),
        Roam::DireHit=>format!("An enemy hero left alive at 35% HP or less takes {}–{}% maximum HP as bonus damage, split equally between physical and magic. Cooldown: {}s.",cfg.dire_hit_min_percent,cfg.dire_hit_max_percent,cfg.dire_hit_cooldown_seconds),
        Roam::None=>return "Roaming: None.".into(),Roam::Auto=>return "Roaming: Selection unavailable.".into(),
    };
    let colour = badge_colour(status.roam).unwrap_or("#ffffffff");
    let progress = status.display(now).0;
    format!(
        "<{colour}>Roaming — {}<>: {effect}\n{progress}.",
        status.roam.label()
    )
}

/// Runtime-only row: saved presets are retained intact. An explicit Roam First
/// choice is an ordering rule, including over existing positional pins.
pub(crate) fn roaming_first_row(
    champion: &str,
    role: crate::build_config::Role,
    choice: Choice,
    raw: &[Option<String>],
) -> Vec<Option<String>> {
    ordered_first_row(champion,role,crate::first_slot_rules::apply(choice,role),raw)
}
fn ordered_first_row(champion:&str,role:crate::build_config::Role,choice:Choice,raw:&[Option<String>])->Vec<Option<String>> {
    let jungle=if choice.jungle_first { jungle_target(champion,role,choice.jungle) }else{None};
    if jungle.is_none() && (!choice.roam_first || !wants_roam_boots(role, choice)) {
        return raw.to_vec();
    }
    let mut row = raw.iter().take(6).cloned().collect::<Vec<_>>();
    row.resize(6, None);
    let boot = if let Some(at) = row
        .iter()
        .position(|s| s.as_deref().is_some_and(|k| if jungle.is_some(){is_jungle_key(k)}else{is_completed_boot(k)}))
    {
        row.remove(at).unwrap()
    } else {
        if let Some(at) = row.iter().rposition(Option::is_none) {
            row.remove(at);
        } else {
            row.pop();
        }
        jungle.map(str::to_string).unwrap_or_else(||crate::build_config::rule_boots(champion)
            .unwrap_or_else(|| crate::smart_builds::boots_for(champion, role, &[]).into()))
    };
    row.insert(0, Some(boot));
    row
}
