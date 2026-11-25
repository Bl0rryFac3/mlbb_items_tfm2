//! Shared MLBB rules. Simulation-owned keys, bounded state, no wall-clock timers.
use mod_api_stable::*;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TimedStack {
    pub count: usize,
    pub expires: usize,
    pub next: usize,
}
impl TimedStack {
    pub fn count(&self, now: usize) -> usize {
        if now < self.expires {
            self.count
        } else {
            0
        }
    }
    pub fn gain(
        &mut self,
        now: usize,
        count: usize,
        cap: usize,
        duration: usize,
        interval: usize,
    ) -> usize {
        if now >= self.expires {
            self.count = 0;
        }
        if now >= self.next {
            self.count = (self.count + count).min(cap);
            self.expires = now + duration;
            self.next = now + interval;
        }
        self.count(now)
    }
}

pub(super) fn inventory(ctx: &StableSim<'_>, entity: usize) -> Vec<String> {
    (0..ctx.player_count())
        .filter_map(|i| ctx.player_at(i))
        .find(|p| p.champion().is_some_and(|e| e.id() == entity))
        .map(|p| p.item_keys())
        .unwrap_or_default()
}
pub(super) fn base(key: &str) -> &str {
    key.strip_prefix("mythical_").unwrap_or(key)
}
pub(crate) fn group(key: &str) -> Option<&'static str> {
    match base(key) {
        "swift_crossbow" | "corrosion_scythe" | "feather_of_heaven" => Some("impulse"),
        "magic_blade" | "rose_gold_meteor" => Some("lifeline"),
        "dreadnaught_armor" | "antique_cuirass" => Some("deter"),
        "molten_essence" | "cursed_helmet" => Some("burning_soul"),
        "black_ice_shield" | "dominance_ice" => Some("fortress"),
        "windtalker" | "legacy_windtalker" => Some("typhoon"),
        "starlium_scythe" | "calamity_reaper" => Some("calamity"),
        "scarlet_phantom" | "haas_claws" => Some("frenzy"),
        "elegant_gem" | "clock_of_destiny" => Some("gift"),
        "flask_of_the_oasis" => Some("blessing"),
        "hunters_knife"|"curse_sword"|"nimble_blade"|"pillager_axe"|"star_shard"|"raptor_machete"|"beast_killer" => Some("jungle"),
        _ => None,
    }
}
fn rank(key: &str) -> (usize, usize) {
    (
        match base(key) {
            "hunters_knife" => 0,
            "curse_sword"|"nimble_blade"|"pillager_axe" => 1,
            "star_shard"|"raptor_machete"|"beast_killer" => 2,
            "swift_crossbow" | "magic_blade" | "dreadnaught_armor" | "molten_essence"
            | "black_ice_shield" | "elegant_gem" | "legacy_windtalker" | "haas_claws" => 0,
            _ => 1,
        },
        key.starts_with("mythical_") as usize,
    )
}
pub(crate) fn selected<'a>(keys: &'a [String], family: &str) -> Option<&'a str> {
    // Equal strength: deterministic first inventory entry; stronger finished effect wins.
    let mut best: Option<&str> = None;
    for key in keys {
        if group(key) == Some(family) && best.is_none_or(|old| rank(key) > rank(old)) {
            best = Some(key);
        }
    }
    best
}
pub(super) fn active(ctx: &StableSim<'_>, entity: usize, key: &str, family: &str) -> bool {
    let keys = inventory(ctx, entity);
    selected(&keys, family).is_none_or(|chosen| chosen == key)
}

pub(super) fn shared_buff(
    ctx: &mut StableSim<'_>,
    entity: usize,
    name: &str,
    mut value: BuffV1,
    duration: usize,
) {
    let old = ctx.get_entity(entity).and_then(|e| {
        (0..e.buff_count())
            .filter_map(|i| e.buff_at(i))
            .find(|b| b.name() == name)
    });
    value.set_name(name);
    value.duration_kind = BuffDurationV1::Time.code();
    value.duration_tick = duration;
    if let Some(old) = old {
        value.duration_tick = value.duration_tick.max(old.duration_tick);
        value.heal_reduce = value.heal_reduce.max(old.heal_reduce);
        value.attack_speed_mult = value.attack_speed_mult.max(old.attack_speed_mult);
    }
    crate::refresh_buff(ctx, entity, name, &value);
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AttackEvent {
    pub repeats: usize,
    pub raw: usize,
}
#[derive(Default)]
struct Cycle {
    tick: usize,
    target: usize,
    count: usize,
    event: AttackEvent,
    seen: bool,
}
#[derive(Default)]
struct Game {
    tick: usize,
    cycles: HashMap<usize, Cycle>,
    next_instance: u64,
    leases: HashMap<(usize, String), Vec<(u64, usize)>>,
}
/// Cloning a registered item creates a new inventory instance, not a second
/// reference to the original instance's unique-effect ownership.
#[derive(Debug, Default)]
pub(super) struct Instance {
    scope: Option<(usize, u64, usize)>,
    id: u64,
}
impl Clone for Instance {
    fn clone(&self) -> Self {
        Self::default()
    }
}
pub(super) fn unique(
    ctx: &StableSim<'_>,
    entity: usize,
    key: &str,
    instance: &mut Instance,
) -> bool {
    let keys = inventory(ctx, entity);
    // Prime incoming shield power before any other item's first callback;
    // equipping Oracle after Blood Wings must not weaken the initial Guard.
    if keys.iter().any(|key| base(key) == "oracle") {
        let bonus = crate::passive_tuning::value("oracle", "received_bonus", 25.);
        note_oracle(ctx, entity, bonus as usize);
    }
    let strongest = keys
        .iter()
        .filter(|k| base(k) == base(key))
        .max_by_key(|k| k.starts_with("mythical_"));
    if strongest.is_some_and(|chosen| chosen != key) {
        return false;
    }
    let game_id = crate::heal_shield_power::game_of(ctx);
    let scope = (game_id, ctx.seed(), entity);
    let now = ctx.tick();
    GAMES.with(|games| {
        let mut games = games.borrow_mut();
        if games.len() >= 16 && !games.contains_key(&(game_id, ctx.seed())) {
            games.clear();
        }
        let game = games.entry((game_id, ctx.seed())).or_default();
        if now < game.tick {
            *game = Game::default();
        }
        game.tick = now;
        let lease_key = (entity, key.to_owned());
        let valid = instance.scope == Some(scope)
            && game
                .leases
                .get(&lease_key)
                .is_some_and(|leases| leases.iter().any(|(id, _)| *id == instance.id));
        if !valid {
            game.next_instance += 1;
            instance.id = game.next_instance;
            instance.scope = Some(scope);
        }
        let leases = game.leases.entry(lease_key).or_default();
        // A missing copy cannot retain ownership forever. One complete tick
        // of grace makes the choice independent of callbacks within a tick.
        leases.retain(|(id, seen)| *id == instance.id || seen.saturating_add(1) >= now);
        if let Some((_, seen)) = leases.iter_mut().find(|(id, _)| *id == instance.id) {
            *seen = now;
        } else {
            leases.push((instance.id, now));
        }
        leases.iter().map(|(id, _)| *id).min() == Some(instance.id)
    })
}
thread_local! {static GAMES:RefCell<HashMap<(usize,u64),Game>>=RefCell::new(HashMap::new());}
pub(super) fn attack_event(
    ctx: &StableSim<'_>,
    caster: usize,
    target: usize,
    damage: usize,
    basic: bool,
    crit: bool,
) -> AttackEvent {
    if !basic {
        return AttackEvent {
            repeats: 1,
            raw: damage,
        };
    }
    let golden = inventory(ctx, caster)
        .iter()
        .any(|k| base(k) == "golden_staff");
    let scope = (crate::heal_shield_power::game_of(ctx), ctx.seed());
    let now = ctx.tick();
    GAMES.with(|games| {
        let mut games = games.borrow_mut();
        if games.len() >= 16 && !games.contains_key(&scope) {
            games.clear();
        }
        let game = games.entry(scope).or_default();
        if now < game.tick {
            *game = Game::default();
        }
        game.tick = now;
        let cycle = game.cycles.entry(caster).or_default();
        // All carried item callbacks for this primary hit see the same event.
        // The host exposes no attack sequence ID: same-tick/target split hits coalesce.
        if !cycle.seen || cycle.tick != now || cycle.target != target {
            cycle.seen = true;
            cycle.tick = now;
            cycle.target = target;
            if golden && !crit {
                cycle.count += 1;
            } else if !golden {
                cycle.count = 0;
            }
            cycle.event = AttackEvent {
                repeats: if golden && !crit && cycle.count % 3 == 0 {
                    3
                } else {
                    1
                },
                raw: damage,
            };
        }
        cycle.event
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_vamp_respects_hit_types_and_absorbed_damage() {
        let mut s = Sustain::default();
        s.note_flat("mythical_feather_of_heaven", 10);
        s.note_flat("mystic_container", 8);
        s.note_flat("war_axe", 8);
        assert_eq!(s.heal(AttackTypeV1::BaseAttack.code(), 150, 100, 50), 18);
        assert_eq!(s.heal(AttackTypeV1::Skill.code(), 150, 100, 50), 16);
        assert_eq!(s.heal(AttackTypeV1::Item.code(), 150, 100, 50), 0);
        assert_eq!(s.heal(AttackTypeV1::BaseAttack.code(), 150, 100, 150), 0);
    }
    #[test]
    fn bloodlust_axe_heals_from_skill_hp_loss_once_per_inventory() {
        let mut sustain = Sustain::default();
        sustain.note("bloodlust_axe");
        sustain.note("bloodlust_axe");
        assert_eq!(sustain.heal(AttackTypeV1::BaseAttack.code(), 150, 100, 40), 0);
        assert_eq!(sustain.heal(AttackTypeV1::Skill.code(), 150, 100, 40), 12);
        assert_eq!(sustain.heal(AttackTypeV1::Skill.code(), 150, 50, 40), 6);
        sustain.note("mythical_bloodlust_axe");
        sustain.note("bloodlust_axe");
        assert_eq!(sustain.heal(AttackTypeV1::Skill.code(), 150, 100, 40), 15);
    }
    #[test]
    fn stacks_expire_and_interval_does_not_extend_a_rejected_gain() {
        let mut s = TimedStack::default();
        assert_eq!(s.gain(0, 1, 3, 120, 24), 1);
        assert_eq!(s.gain(1, 1, 3, 120, 24), 1);
        assert_eq!(s.expires, 120);
        assert_eq!(s.gain(24, 1, 3, 120, 24), 2);
        assert_eq!(s.count(144), 0);
        assert_eq!(s.gain(144, 1, 3, 120, 24), 1);
    }
    #[test]
    fn stronger_shared_effect_wins_and_unrelated_items_do_not_interfere() {
        let a = vec![
            "swift_crossbow".into(),
            "feather_of_heaven".into(),
            "mythical_corrosion_scythe".into(),
            "golden_staff".into(),
        ];
        assert_eq!(selected(&a, "impulse"), Some("mythical_corrosion_scythe"));
        assert_eq!(selected(&a, "lifeline"), None);
        assert_eq!(group("mythical_radiant_armor"), None);
    }
}

#[derive(Clone, Copy, Default)]
struct Recipient {
    oracle: usize,
    oracle_until: usize,
    cut_until: usize,
    physical_until: usize,
    athena_owned_until: usize,
    athena_window: usize,
    athena_last_hit: Option<usize>,
    athena_values: Option<(usize, usize, usize)>,
    guardian_until: usize,
    guardian_hp: usize,
    immortal_until: usize,
    immortal_rescue_until: usize,
    stasis_until: usize,
}
#[derive(Default)]
struct DeterState {
    stacks: TimedStack,
    step: usize,
    applied: usize,
    previous: usize,
}
impl DeterState {
    fn reduction(&self, now: usize) -> usize {
        if now == self.applied {
            self.previous
        } else {
            self.stacks.count(now) * self.step
        }
    }
    fn apply(&mut self, now: usize, step: usize) {
        let previous = self.reduction(now);
        if self.stacks.count(now) == 0 {
            self.step = step;
        } else {
            self.step = self.step.max(step);
        }
        self.stacks.gain(now, 1, 3, crate::ticks(2.), 0);
        self.applied = now;
        self.previous = previous;
    }
}
#[derive(Clone, Copy)]
pub(super) struct ImmortalRecovery {
    pub due: usize,
    pub expires: usize,
    pub needs_hp: bool,
    pub needs_shield: bool,
}

const IMMORTAL_HANDOFF_TICKS: usize = 3;

#[derive(Default)]
struct RecipientGame {
    tick: usize,
    units: HashMap<usize, Recipient>,
    deter: HashMap<usize, DeterState>,
    penetration: HashMap<usize, Penetration>,
    sustain: HashMap<usize, (usize, Sustain)>,
    pending_sustain: Vec<SustainHeal>,
    pending_recovery: Vec<(usize, usize, usize)>,
    cooldowns: HashMap<(usize, String), usize>,
    item_memory: HashMap<(usize, String), (usize, usize)>,
    immortal_hits: HashMap<usize, usize>,
    immortal_recoveries: HashMap<usize, ImmortalRecovery>,
    twilight: HashMap<usize, TwilightState>,
    next_aid: usize,
    aids: Vec<(usize, usize, usize, usize)>,
}
thread_local! {static RECIPIENTS:RefCell<HashMap<usize,RecipientGame>>=RefCell::new(HashMap::new());}
pub(crate) fn on_match_start(ctx: &StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        r.borrow_mut().remove(&game);
    });
    GAMES.with(|r| {
        r.borrow_mut().retain(|(id, _), _| *id != game);
    });
}
pub(crate) fn on_match_tick(ctx: &StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        {
            let g = r.entry(game).or_default();
            if now < g.tick {
                *g = RecipientGame::default();
            }
            g.tick = now;
            g.units.retain(|_, u| {
                now < u.oracle_until
                    || now < u.cut_until
                    || now < u.physical_until
                    || now < u.athena_owned_until
                    || now < u.guardian_until
                    || now < u.immortal_until
                    || now < u.immortal_rescue_until
                    || now < u.stasis_until
            });
            g.twilight.retain(|id, state| now < state.equipped_until
                || inventory(ctx, *id).iter().any(|key| base(key) == "twilight_armor"));
            g.immortal_hits.retain(|_, at| now.saturating_sub(*at) < IMMORTAL_HANDOFF_TICKS);
            g.immortal_recoveries.retain(|_, recovery| now <= recovery.expires);
            g.deter.retain(|_, state| state.stacks.count(now) > 0);
            g.item_memory
                .retain(|(entity, key), _| inventory(ctx, *entity).iter().any(|k| base(k) == key));
        }
    });
}
pub(super) fn note_oracle(ctx: &StableSim<'_>, id: usize, percent: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        if now < g.tick {
            *g = RecipientGame::default();
        }
        g.tick = now;
        let u = g.units.entry(id).or_default();
        u.oracle = percent;
        u.oracle_until = now + 2;
    });
}
pub(super) fn note_lifebane(ctx: &StableSim<'_>, id: usize, until: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        if now < g.tick {
            *g = RecipientGame::default();
        }
        g.tick = now;
        let u = g.units.entry(id).or_default();
        u.cut_until = u.cut_until.max(until);
    });
}
pub(crate) fn recipient_gain(game: usize, target: usize, amount: usize, shield: bool) -> usize {
    RECIPIENTS.with(|r| {
        let r = r.borrow();
        let Some(g) = r.get(&game) else { return amount };
        let Some(u) = g.units.get(&target) else {
            return amount;
        };
        let oracle = if g.tick < u.oracle_until { u.oracle } else { 0 };
        let cut = if shield && g.tick < u.cut_until {
            40
        } else {
            0
        };
        gain(amount, oracle, cut)
    })
}
fn gain(amount: usize, oracle: usize, cut: usize) -> usize {
    ((amount as u128 * (100 + oracle.min(100)) as u128 * (100 - cut.min(100)) as u128 + 5000)
        / 10000)
        .min(usize::MAX as u128) as usize
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Penetration {
    flat_ad: usize,
    flat_ap: usize,
    armor_buster: bool,
    malefic_roar: bool,
    divine_glaive: bool,
    ad_bonus: Option<(usize, usize, usize, usize)>,
    ap_bonus: Option<(usize, usize, usize, usize)>,
}
// Damage-based healing without enabling engine omnivamp.
#[derive(Clone, Copy, Default)]
struct Sustain {
    lifesteal: usize,
    hybrid: usize,
    spell_vamp: usize,
    flat: [usize; 3],
}

// Basic-hit, skill-hit, or either. Items outside this table keep native vamp.
pub(super) fn sustain_kind(key: &str) -> Option<usize> {
    match base(key) {
        "feather_of_heaven" | "wind_of_nature" | "vampire_mallet" | "magic_blade"
        | "rose_gold_meteor" => Some(0),
        "ice_queen_wand" | "mystic_container" | "queens_wings" | "star_shard" => Some(1),
        "war_axe" | "starlium_scythe" | "shadow_twinblades" => Some(2),
        _ => None,
    }
}

static STAT_SUSTAIN: std::sync::OnceLock<HashMap<String, usize>> = std::sync::OnceLock::new();

pub(super) fn register_sustain(stats: HashMap<String, usize>) {
    let _ = STAT_SUSTAIN.set(stats);
}

pub(super) fn grants_sustain(key: &str) -> bool {
    matches!(base(key), "haas_claws" | "concentrated_energy" | "bloodlust_axe")
}

impl Sustain {
    fn note(&mut self, key: &str) {
        let family = base(key);
        let scale = if family == key { 1. } else { 1.25 };
        match family {
            "haas_claws" => {
                let rate = crate::passive_tuning::value(family, "lifesteal", 12.);
                self.lifesteal = self.lifesteal.max((rate * scale).round() as usize);
            }
            "concentrated_energy" => {
                let rate = crate::passive_tuning::value(family, "lifesteal", 12.);
                self.hybrid = self.hybrid.max((rate * scale).round() as usize);
            }
            "bloodlust_axe" => {
                let rate = crate::passive_tuning::value(family, "spell_vamp", 12.);
                self.spell_vamp = self.spell_vamp.max((rate * scale).round() as usize);
            }
            _ => {}
        }
    }

    fn note_flat(&mut self, key: &str, rate: usize) {
        if let Some(kind) = sustain_kind(key) {
            self.flat[kind] += rate;
        }
    }

    fn heal(self, attack_type: u32, damage: usize, hp: usize, shield: usize) -> usize {
        let percent = match AttackTypeV1::from_code(attack_type) {
            Some(AttackTypeV1::BaseAttack) => self.lifesteal + self.hybrid + self.flat[0] + self.flat[2],
            Some(AttackTypeV1::Skill) => self.hybrid + self.spell_vamp + self.flat[1] + self.flat[2],
            _ => return 0,
        };
        let lost_hp = damage.saturating_sub(shield).min(hp);
        (lost_hp as u128 * percent as u128 / 100) as usize
    }
}

struct SustainHeal {
    source: usize,
    target: usize,
    amount: usize,
    hit_tick: usize,
}

pub(crate) fn note_sustain_hit(
    game: usize,
    source_token: usize,
    target: usize,
    tick: usize,
    attack_type: u32,
    damage: usize,
    hp: usize,
    shield: usize,
) {
    if damage == 0 || hp == 0 {
        return;
    }
    RECIPIENTS.with(|all| {
        let mut all = all.borrow_mut();
        let Some(state) = all.get_mut(&game) else { return };
        let Some(&(source, sustain)) = state.sustain.get(&source_token) else { return };
        let amount = sustain.heal(attack_type, damage, hp, shield);
        if source != target && amount > 0 {
            state.pending_sustain.push(SustainHeal {
                source,
                target,
                amount,
                hit_tick: tick,
            });
        }
    });
}

pub(super) fn recover_after_landing(ctx: &mut StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    let due = RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        let Some(g) = r.get_mut(&game) else {
            return Vec::new();
        };
        let mut due = Vec::new();
        g.pending_recovery.retain(|&(target, amount, at)| {
            if now >= at {
                due.push((target, amount));
                false
            } else {
                true
            }
        });
        due
    });
    for (id, amount) in due {
        if ctx.get_entity(id).is_some_and(|e| e.is_alive()) {
            crate::heal_shield_power::unraised(|| ctx.heal(id, id, amount));
        }
    }
}
pub(super) fn recover_sustain(ctx: &mut StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    let due = RECIPIENTS.with(|all| {
        let mut all = all.borrow_mut();
        let Some(state) = all.get_mut(&game) else { return Vec::new() };
        let count = state.pending_sustain.partition_point(|hit| hit.hit_tick < now);
        state.pending_sustain.drain(..count).collect::<Vec<_>>()
    });
    for hit in due {
        let Some(source) = ctx.get_entity(hit.source).filter(|e| e.is_alive()) else {
            continue;
        };
        let Some(target) = ctx.get_entity(hit.target) else { continue };
        if target.team() != source.team()
            && !target.is_tower()
            && target.kind() != Some(EntityKindV1::Nexus)
        {
            crate::heal_shield_power::unraised(|| ctx.heal(hit.source, hit.source, hit.amount));
        }
    }
}

pub(super) fn note_inventory(ctx: &StableSim<'_>) {
    let track_penetration = crate::tactics::heal_shield::penetration_installed();
    let track_sustain = crate::tactics::heal_shield::landing_installed();
    if !track_penetration && !track_sustain {
        return;
    }
    let game = crate::heal_shield_power::game_of(ctx);
    let mut entries = HashMap::new();
    let mut sustain_entries = HashMap::new();
    for i in 0..ctx.player_count() {
        let Some(p) = ctx.player_at(i) else {
            continue;
        };
        let Some(id) = p.champion().map(|e| e.id()) else {
            continue;
        };
        let Some(token) = crate::tactics::heal_shield::entity_token(game, id) else {
            continue;
        };
        let mut pen = Penetration::default();
        let mut sustain = Sustain::default();
        // Repeated copies of a unique item do not duplicate its unique PEN.
        let mut seen = std::collections::HashSet::new();
        for key in p.item_keys() {
            if track_sustain {
                sustain.note(&key);
                if let Some(rate) = STAT_SUSTAIN.get().and_then(|stats| stats.get(&key)) {
                    let configured = crate::patches::live::config_for(&key)
                        .and_then(|config| config.vamp)
                        .map_or(*rate, |value| value.max(0) as usize);
                    let family = base(&key);
                    let field = match family {
                        "vampire_mallet" => Some("lifesteal"),
                        "mystic_container" => Some("spell_vamp"),
                        _ => None,
                    };
                    let configured = field.map_or(configured, |field| {
                        crate::passive_tuning::value(family, field, configured as f64) as usize
                    });
                    sustain.note_flat(&key, configured);
                }
            }
            let base = base(&key);
            if !track_penetration || !seen.insert(base.to_owned()) {
                continue;
            }
            if let Some(s) = super::specs().iter().find(|s| s.key == base) {
                pen.flat_ad += s
                    .stats
                    .get("flat_physical_penetration")
                    .copied()
                    .unwrap_or(0)
                    .max(0) as usize;
                pen.flat_ap += s
                    .stats
                    .get("flat_magic_penetration")
                    .copied()
                    .unwrap_or(0)
                    .max(0) as usize;
            }
            pen.armor_buster |= matches!(base, "malefic_roar" | "malefic_gun");
            pen.malefic_roar |= base == "malefic_roar";
            pen.divine_glaive |= base == "divine_glaive";
            if base == "malefic_roar" && pen.ad_bonus.is_none() {
                pen.ad_bonus = Some(penetration_settings(base, 30., 30.));
            }
            if base == "divine_glaive" && pen.ap_bonus.is_none() {
                pen.ap_bonus = Some(penetration_settings(base, 40., 20.));
            }

        }
        if track_penetration {
            entries.insert(token, pen);
        }
        if sustain.lifesteal > 0 || sustain.hybrid > 0 || sustain.spell_vamp > 0 || sustain.flat != [0; 3] {
            sustain_entries.insert(token, (id, sustain));
        }
    }
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        g.tick = ctx.tick();
        g.penetration = entries;
        g.sustain = sustain_entries;
    });
}
fn penetration_settings(key: &str, base: f64, cap: f64) -> (usize, usize, usize, usize) {
    let value = |id, default| crate::passive_tuning::value(key, id, default) as usize;
    (value("base_pen", base), value("per_step", 1.), value("step", 10.).max(1), value("cap", cap))
}

fn penetration_bonus(defence: usize, (base, per_step, step, cap): (usize, usize, usize, usize)) -> usize {
    base + (defence / step).saturating_mul(per_step).min(cap)
}

pub(crate) fn penetration_multiplier(
    game: usize,
    token: usize,
    defence: usize,
    native_percent: usize,
    kind: u32,
) -> f64 {
    RECIPIENTS.with(|r| {
        let r = r.borrow();
        let Some(p) = r.get(&game).and_then(|g| g.penetration.get(&token)) else {
            return 1.;
        };
        let native = defence.saturating_mul(100usize.saturating_sub(native_percent.min(100))) / 100;
        let (flat, percent) = match kind {
            0 => (
                p.flat_ad,
                if p.armor_buster {
                    if p.malefic_roar {
                        penetration_bonus(defence, p.ad_bonus.unwrap_or((30, 1, 10, 30)))
                    } else {
                        30
                    }
                } else {
                    0
                },
            ),
            1 => (
                p.flat_ap,
                if p.divine_glaive {
                    penetration_bonus(defence, p.ap_bonus.unwrap_or((40, 1, 10, 20)))
                } else {
                    0
                },
            ),
            _ => return 1.,
        };
        let effective = (native * (100 - percent.min(100)) / 100).saturating_sub(flat);
        (100 + native) as f64 / (100 + effective) as f64
    })
}
pub(super) fn claim_cooldown(
    ctx: &StableSim<'_>,
    entity: usize,
    family: &str,
    duration: usize,
) -> bool {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        g.tick = now;
        let until = g.cooldowns.entry((entity, family.to_owned())).or_default();
        if now < *until {
            false
        } else {
            *until = now.saturating_add(duration);
            true
        }
    })
}
pub(super) fn item_ready(ctx: &StableSim<'_>, entity: usize, key: &str) -> usize {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        r.borrow()
            .get(&game)
            .and_then(|g| {
                g.cooldowns
                    .get(&(entity, format!("item:{}", base(key))))
                    .copied()
            })
            .unwrap_or(0)
    })
}
pub(super) fn keep_item_ready(ctx: &StableSim<'_>, entity: usize, key: &str, until: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        r.entry(game)
            .or_default()
            .cooldowns
            .insert((entity, format!("item:{}", base(key))), until);
    });
}
pub(super) fn item_memory(ctx: &StableSim<'_>, entity: usize, key: &str) -> Option<(usize, usize)> {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        r.borrow()
            .get(&game)
            .and_then(|g| g.item_memory.get(&(entity, base(key).to_owned())).copied())
    })
}
pub(super) fn keep_item_memory(
    ctx: &StableSim<'_>,
    entity: usize,
    key: &str,
    stacks: usize,
    revive: usize,
) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        r.entry(game)
            .or_default()
            .item_memory
            .insert((entity, base(key).to_owned()), (stacks, revive));
    });
}
pub(super) fn clear_transient(ctx: &StableSim<'_>, entity: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        if let Some(g) = r.borrow_mut().get_mut(&game) {
            g.units.remove(&entity);
            g.immortal_hits.remove(&entity);
            g.immortal_recoveries.remove(&entity);
            g.pending_recovery.retain(|(id, _, _)| *id != entity);
        }
    });
}
pub(crate) fn note_aid(game: usize, caster: usize, target: usize) {
    if caster == target {
        return;
    }
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        g.next_aid = g.next_aid.saturating_add(1);
        if g.aids.len() >= 256 {
            g.aids.drain(..128);
        }
        g.aids.push((g.next_aid, caster, target, g.tick));
    });
}
pub(crate) fn aid_events(
    ctx: &StableSim<'_>,
    caster: usize,
    after: usize,
) -> (usize, Vec<(usize, usize)>) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let r = r.borrow();
        let Some(g) = r.get(&game) else {
            return (after, Vec::new());
        };
        (
            g.next_aid,
            g.aids
                .iter()
                .filter(|(seq, c, _, tick)| {
                    *seq > after
                        && *c == caster
                        && ctx.tick().saturating_sub(*tick) <= crate::ticks(3.)
                })
                .map(|(_, _, target, tick)| (*target, *tick))
                .collect(),
        )
    })
}
pub(super) fn note_protection(ctx: &StableSim<'_>, id: usize, kind: &str, until: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        g.tick = ctx.tick();
        let u = g.units.entry(id).or_default();
        match kind {
            "physical" => u.physical_until = u.physical_until.max(until),
            #[cfg(test)]
            "athena" => u.athena_owned_until = until,
            "guardian" => {
                u.guardian_until = until;
                u.guardian_hp = ctx.get_entity(id).map(|e| e.hp().1).unwrap_or(0);
            }
            "immortality" => u.immortal_until = until,
            "stasis" => u.stasis_until = until,
            _ => {}
        }
    });
}
/// Called at the verified native damage landing point, after our typed damage
/// policies. Reaching one HP naturally must not spend Immortality.
pub(super) fn note_magic_guard(ctx: &StableSim<'_>, id: usize, reduction: usize, duration: usize, cooldown: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|state| {
        let mut state = state.borrow_mut();
        if state.len() >= 16 && !state.contains_key(&game) {
            state.clear();
        }
        let game = state.entry(game).or_default();
        game.tick = now;
        let unit = game.units.entry(id).or_default();
        unit.athena_owned_until = now.saturating_add(2);
        unit.athena_values = Some((reduction, duration, cooldown));
    });
}

pub(crate) fn note_immortal_hit(
    game: usize, entity: usize, now: usize, hp: usize, shield: usize, damage: usize,
) -> usize {
    if hp == 0 || damage == 0 {
        return damage;
    }
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        let Some(g) = r.get_mut(&game) else { return damage };
        let Some(u) = g.units.get_mut(&entity) else { return damage };
        let total_hp = shield.saturating_add(hp);
        if now < u.immortal_rescue_until {
            return damage.min(total_hp.saturating_sub(1));
        }
        if now < u.immortal_until && damage >= total_hp {
            u.immortal_until = 0;
            u.immortal_rescue_until = now.saturating_add(IMMORTAL_HANDOFF_TICKS);
            g.immortal_hits.insert(entity, now);
            crate::tactics::heal_shield::immortality_status(|| format!(
                "MLBB Immortality intercepted lethal hit: unit={entity} tick={now} hp={hp} shield={shield} damage={damage}"
            ));
            return total_hp.saturating_sub(1);
        }
        damage
    })
}

pub(super) fn take_immortal_hit(ctx: &StableSim<'_>, entity: usize) -> bool {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        r.borrow_mut()
            .get_mut(&game)
            .and_then(|g| g.immortal_hits.remove(&entity))
            .is_some_and(|at| ctx.tick() >= at && ctx.tick() - at < IMMORTAL_HANDOFF_TICKS)
    })
}

pub(super) fn immortal_recovery(ctx: &StableSim<'_>, entity: usize) -> Option<ImmortalRecovery> {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| r.borrow().get(&game)?.immortal_recoveries.get(&entity).copied())
}

pub(super) fn set_immortal_recovery(
    ctx: &StableSim<'_>, entity: usize, recovery: Option<ImmortalRecovery>,
) {
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        let Some(g) = r.get_mut(&game) else { return };
        if let Some(recovery) = recovery {
            g.immortal_recoveries.insert(entity, recovery);
        } else {
            g.immortal_recoveries.remove(&entity);
        }
    });
}

pub(super) fn note_deter(ctx: &StableSim<'_>, attacker: usize, step: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    let Some(token) = crate::tactics::heal_shield::entity_token(game, attacker) else {
        return;
    };
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() >= 16 && !r.contains_key(&game) {
            r.clear();
        }
        let g = r.entry(game).or_default();
        g.tick = ctx.tick();
        let e = g.deter.entry(token).or_default();
        e.apply(g.tick, step);
    });
}
struct TwilightState {
    preset: crate::passive_presets::Preset,
    max_hp: usize,
    equipped_until: usize,
    next_refresh: usize,
    enemy_tokens: Vec<usize>,
    active_until: usize,
    ready: usize,
    next_pulse: usize,
}

pub(super) fn note_twilight(
    ctx: &StableSim<'_>,
    id: usize,
    max_hp: usize,
    preset: crate::passive_presets::Preset,
) {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| {
        let mut held = r.borrow_mut();
        let entry = held.entry(game).or_default().twilight.entry(id).or_insert_with(|| TwilightState {
            preset, max_hp, equipped_until: 0, next_refresh: 0, enemy_tokens: Vec::new(),
            active_until: 0, ready: 0, next_pulse: 0,
        });
        entry.preset = preset;
        entry.max_hp = max_hp;
        entry.equipped_until = now.saturating_add(2);
        if now >= entry.next_refresh {
            let Some(team) = ctx.get_entity(id).map(|e| e.team()) else { return; };
            entry.enemy_tokens = (0..ctx.entity_count()).filter_map(|i| ctx.entity_at(i))
                .filter(|entity| entity.team() != team)
                .filter_map(|entity| crate::tactics::heal_shield::entity_token(game, entity.id()))
                .collect();
            entry.next_refresh = now.saturating_add(crate::ticks(0.25));
        }
    });
}

pub(super) fn take_defiance_pulse(ctx: &StableSim<'_>, id: usize) -> bool {
    let now = ctx.tick();
    let game = crate::heal_shield_power::game_of(ctx);
    RECIPIENTS.with(|r| {
        let mut held = r.borrow_mut();
        let Some(state) = held.get_mut(&game).and_then(|game| game.twilight.get_mut(&id)) else {
            return false;
        };
        if state.next_pulse == 0 || now < state.next_pulse || now > state.active_until {
            return false;
        }
        state.next_pulse = state.next_pulse.saturating_add(crate::ticks(1.));
        true
    })
}

impl TwilightState {
    fn reduce(&mut self, now: usize, source: Option<usize>, damage_type: u32, amount: usize) -> usize {
        use crate::passive_presets::Preset;
        if now >= self.equipped_until || !source.is_some_and(|token| self.enemy_tokens.contains(&token)) {
            return amount;
        }
        match self.preset {
            Preset::Twilight { threshold, scale } => {
                let excess = amount.saturating_sub(threshold.into());
                let fraction = (self.max_hp as f64 * scale / 10_000.).clamp(0., 1.);
                amount - (excess as f64 * fraction).round() as usize
            }
            Preset::Defiance { threshold, reduction, seconds, cooldown, .. } if damage_type == 0 => {
                if amount > usize::from(threshold) && now >= self.ready && now >= self.active_until {
                    self.active_until = now.saturating_add(crate::ticks(seconds.into()));
                    self.ready = now.saturating_add(crate::ticks(cooldown));
                    self.next_pulse = now.saturating_add(crate::ticks(1.));
                }
                if now < self.active_until {
                    amount - (amount as f64 * f64::from(reduction) / 100.).round() as usize
                } else {
                    amount
                }
            }
            _ => amount,
        }
    }
}

// Called at the verified engine shield/HP landing function, after outgoing
// item callbacks. Zero physical damage here cannot be re-added by an on-hit proc.
pub(crate) fn landing_damage(
    game: usize,
    target: usize,
    source_token: Option<usize>,
    amount: usize,
    damage_type: u32,
) -> usize {
    RECIPIENTS.with(|r| {
        let mut r = r.borrow_mut();
        let Some(g) = r.get_mut(&game) else {
            return amount;
        };
        let now = g.tick;
        let mut result = amount;
        if damage_type == 0 {
            if let Some(state) = source_token.and_then(|t| g.deter.get(&t)) {
                result = gain(result, 0, state.reduction(now));
            }
        }
        if let Some(u) = g.units.get_mut(&target) {
            if now<u.stasis_until {return 0;}
            if damage_type == 0 && now < u.physical_until {
                return 0;
            }
            if damage_type == 1 && amount > 0 && now < u.athena_owned_until {
                let (reduction, duration, cooldown) = u.athena_values.unwrap_or((25, crate::ticks(3.), crate::ticks(5.)));
                if u.athena_last_hit.is_none_or(|last| now.saturating_sub(last) >= cooldown) {
                    u.athena_window = now.saturating_add(duration);
                }
                u.athena_last_hit = Some(now);
                if now < u.athena_window {
                    result = gain(result, 0, reduction);
                }
            }
            if now < u.guardian_until && result > 175 {
                let refund = ((result - 175) as f64
                    * (0.3 + u.guardian_hp as f64 * 0.00006).min(1.))
                .round() as usize;
                g.pending_recovery.push((target, refund, now + 1));
            }
        }
        if let Some(state) = g.twilight.get_mut(&target) {
            result = state.reduce(now, source_token, damage_type, result);
        }
        result
    })
}
pub(super) fn shield(ctx: &mut StableSim<'_>, id: usize, amount: usize, duration: usize) -> bool {
    let amount = if crate::tactics::heal_shield::sdk_shield_installed() {
        amount
    } else {
        recipient_gain(crate::heal_shield_power::game_of(ctx), id, amount, true)
    };
    ctx.entity_add_shield(id, amount, duration)
}
#[cfg(test)]
mod recipient_tests {
    use super::*;

    #[test]
    fn sustain_uses_the_strongest_copy_of_each_item_and_eligible_hits_only() {
        let mut sustain = Sustain::default();
        sustain.note("mythical_haas_claws");
        sustain.note("haas_claws");
        sustain.note("mythical_haas_claws");
        let basic = AttackTypeV1::BaseAttack.code();
        let skill = AttackTypeV1::Skill.code();
        assert_eq!(sustain.heal(basic, 100, 100, 0), 15);
        assert_eq!(sustain.heal(skill, 100, 100, 0), 0);

        sustain.note("concentrated_energy");
        assert_eq!(sustain.heal(skill, 100, 100, 0), 12);
        assert_eq!(sustain.heal(basic, 100, 100, 0), 27);
        sustain.note("mythical_concentrated_energy");
        assert_eq!(sustain.heal(skill, 100, 100, 0), 15);
        for attack in [AttackTypeV1::Item, AttackTypeV1::Dot, AttackTypeV1::DotIgnoreShield, AttackTypeV1::Well] {
            assert_eq!(sustain.heal(attack.code(), 100, 100, 0), 0);
        }
    }

    #[test]
    fn sustain_queues_only_hp_lost_after_shields_and_overkill() {
        let game = 5020;
        let basic = AttackTypeV1::BaseAttack.code();
        let mut sustain = Sustain::default();
        sustain.note("haas_claws");
        RECIPIENTS.with(|all| {
            all.borrow_mut().entry(game).or_default().sustain.insert(100, (1, sustain));
        });
        note_sustain_hit(game, 100, 2, 30, basic, 100, 100, 100);
        note_sustain_hit(game, 100, 1, 30, basic, 100, 100, 0);
        note_sustain_hit(game, 999, 2, 30, basic, 100, 100, 0);
        note_sustain_hit(game, 100, 2, 30, basic, 300, 50, 20);
        RECIPIENTS.with(|all| {
            let state = all.borrow_mut().remove(&game).unwrap();
            let [hit] = state.pending_sustain.as_slice() else { panic!("expected one queued heal") };
            assert_eq!((hit.source, hit.target, hit.amount, hit.hit_tick), (1, 2, 6, 30));
        });
    }
    #[test]
    fn recipient_modifiers_preserve_identity_and_do_not_delete_existing_layers() {
        assert_eq!(gain(100, 25, 0), 125);
        assert_eq!(gain(100, 25, 40), 75);
        assert_eq!(gain(100, 0, 40), 60);
        assert_eq!(gain(0, 25, 40), 0);
    }
}

#[cfg(test)]
mod damage_tests {
    use super::*;
    #[test]
    fn flat_and_percent_penetration_are_distinct_and_true_damage_is_unchanged() {
        let mut g = RecipientGame::default();
        g.penetration.insert(
            77,
            Penetration {
                flat_ad: 20,
                ..Default::default()
            },
        );
        RECIPIENTS.with(|r| {
            r.borrow_mut().insert(888, g);
        });
        assert!((penetration_multiplier(888, 77, 100, 0, 0) - 200. / 180.).abs() < 1e-9);
        assert!((penetration_multiplier(888, 77, 100, 50, 0) - 150. / 130.).abs() < 1e-9);
        assert_eq!(penetration_multiplier(888, 77, 100, 0, 2), 1.);
        assert_eq!(penetration_multiplier(889, 77, 100, 0, 0), 1.);
    }
    #[test]
    fn refreshing_deter_preserves_the_previous_stacks_for_the_triggering_tick() {
        let mut s = DeterState::default();
        s.apply(0, 8);
        assert_eq!(s.reduction(0), 0);
        assert_eq!(s.reduction(1), 8);
        s.apply(1, 8);
        assert_eq!(s.reduction(1), 8);
        assert_eq!(s.reduction(2), 16);
        assert_eq!(s.reduction(121), 0);
    }
    #[test]
    fn deter_is_typed_and_guardian_queues_a_refund_without_preventing_damage() {
        let mut g = RecipientGame {
            tick: 1,
            ..Default::default()
        };
        g.deter.insert(
            77,
            DeterState {
                stacks: TimedStack {
                    count: 3,
                    expires: 120,
                    next: 0,
                },
                step: 8,
                applied: 0,
                previous: 0,
            },
        );
        g.units.insert(
            0,
            Recipient {
                guardian_until: 3,
                ..Default::default()
            },
        );
        RECIPIENTS.with(|r| {
            r.borrow_mut().insert(777, g);
        });
        assert_eq!(landing_damage(777, 1, Some(77), 100, 0), 76);
        assert_eq!(landing_damage(777, 1, Some(77), 100, 1), 100);
        // The lethal 275 hit remains lethal. A refund is processed only if alive next tick.
        assert_eq!(landing_damage(777, 0, None, 275, 0), 275);
        RECIPIENTS.with(|r| assert_eq!(r.borrow()[&777].pending_recovery, vec![(0, 30, 2)]));
    }
}

pub(super) fn immortal_recovery_pending(ctx: &StableSim<'_>, entity: usize) -> bool {
    let game = crate::heal_shield_power::game_of(ctx);
    let now = ctx.tick();
    RECIPIENTS.with(|r| r.borrow().get(&game).is_some_and(|g| {
        g.immortal_hits.get(&entity).is_some_and(|at| {
            now >= *at && now - *at < IMMORTAL_HANDOFF_TICKS
        }) || g.immortal_recoveries.get(&entity).is_some_and(|recovery| now < recovery.due)
    }))
}

#[cfg(test)]
mod twilight_tests {
    use super::*;
    use crate::passive_presets::Preset;

    fn state(preset: Preset) -> TwilightState {
        TwilightState {
            preset,
            max_hp: 5000,
            equipped_until: usize::MAX,
            next_refresh: 0,
            enemy_tokens: vec![123],
            active_until: 0,
            ready: 0,
            next_pulse: 0,
        }
    }

    #[test]
    fn twilight_only_reduces_enemy_damage_above_the_threshold() {
        let mut policy = state(Preset::Twilight { threshold: 800, scale: 0.4 });
        assert_eq!(policy.reduce(1, Some(123), 0, 800), 800);
        assert_eq!(policy.reduce(1, Some(123), 1, 1800), 1600);
        assert_eq!(policy.reduce(1, Some(456), 1, 1800), 1800);
    }

    #[test]
    fn defiance_requires_a_large_physical_hit_and_respects_its_cooldown() {
        let mut policy = state(Preset::Defiance {
            threshold: 600,
            reduction: 20,
            hp_percent: 100.,
            seconds: 3,
            cooldown: 6.,
        });
        assert_eq!(policy.reduce(1, Some(123), 1, 700), 700);
        assert_eq!(policy.reduce(1, Some(123), 0, 600), 600);
        assert_eq!(policy.reduce(2, Some(123), 0, 700), 560);
        assert_eq!(policy.reduce(3, Some(123), 0, 100), 80);
        assert_eq!(policy.reduce(2 + crate::ticks(4.), Some(123), 0, 700), 700);
        assert_eq!(policy.reduce(2 + crate::ticks(6.), Some(123), 0, 700), 560);
    }
}
