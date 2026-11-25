//! MLBB equipment on the stable simulation API. Native build hooks remain upstream.
//! TFM2 has no mana pool or manual item button: all substitutions are documented.
use crate::{refresh_buff, ticks, ProcQueue};
use crate::passive_presets::{self, Preset};
use mod_api_stable::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
#[path = "mlbb_rules.rs"]
pub(crate) mod rules;
use rules::TimedStack;
#[path = "mlbb_blood.rs"]
mod blood;

#[derive(Debug, Deserialize)]
struct Spec {
    key: String,
    name: String,
    tier: usize,
    price: usize,
    stats: HashMap<String, i32>,
    previous: Vec<String>,
    next: Vec<String>,
    mythical: bool,
    boots: bool,
    category: String,
    #[serde(default)]
    legacy: bool,
}
fn specs() -> &'static [Spec] {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        serde_json::from_str(include_str!("../catalog/equipment.json"))
            .expect("validated MLBB catalog")
    })
}
pub(crate) fn catalog_rows() -> impl Iterator<Item = (&'static str, &'static str, usize, bool, bool)> {
    specs().iter().map(|spec| (
        spec.key.as_str(), spec.category.as_str(), spec.tier + 1, spec.legacy, spec.mythical,
    ))
}

pub(crate) fn catalog_recipe(key: &str) -> Option<(Vec<&'static str>, Vec<&'static str>)> {
    let base = key.strip_prefix("mythical_").unwrap_or(key);
    let spec = specs().iter().find(|spec| spec.key == base)?;
    if key != base {
        return spec.mythical.then_some((vec![spec.key.as_str()], Vec::new()));
    }
    Some((
        spec.previous.iter().map(String::as_str).collect(),
        spec.next.iter().map(String::as_str).collect(),
    ))
}

pub(crate) fn active_key(key: &str) -> bool {
    let (base, mythical) = key
        .strip_prefix("mythical_")
        .map_or((key, false), |base| (base, true));
    specs()
        .iter()
        .any(|s| s.key == base && !s.legacy && (!mythical || s.mythical))
}
#[derive(Clone, Debug)]
pub struct MlbbItem {
    spec: &'static Spec,
    upgraded: bool,
    stats: BuffV1,
    price: usize,
    procs: ProcQueue,
    true_hits: Vec<(usize, usize)>,
    executions: Vec<(usize, usize)>,
    burns: BTreeMap<usize, (usize, usize)>,
    ready: usize,
    secondary: usize,
    last_damage: usize,
    last_attack: usize,
    target: usize,
    count: usize,
    stacks: usize,
    permanent: usize,
    armed: usize,
    last_ult: usize,
    last_skills: (usize, usize),
    revive_at: usize,
    last_tick: usize,
    combo: TimedStack,
    target_stacks: BTreeMap<usize, TimedStack>,
    accumulators: BTreeMap<usize, usize>,
    recoveries: Vec<(usize, usize, usize, usize)>,
    flask_windows: BTreeMap<usize, (usize, usize)>,
    flask_cooldowns: BTreeMap<usize, usize>,
    last_aid_seq: usize,
    speed_until: usize,
    converted_base_crit: Option<usize>,
    instance: rules::Instance,
}
pub fn make(key: &str, upgraded: bool, config: Option<&crate::config::ItemConfig>) -> MlbbItem {
    let spec = specs()
        .iter()
        .find(|s| s.key == key)
        .expect("registered MLBB key");
    assert!(!upgraded || spec.mythical);
    let mut stats = BuffV1::default();
    macro_rules! stat { ($($f:ident),* $(,)?) => { $(stats.$f=spec.stats.get(stringify!($f)).copied().unwrap_or(0).max(0) as _;)* }; }
    stat!(
        attack,
        magic_power,
        hp,
        hp_regen,
        defence,
        magic_resistance,
        vamp,
        crit_chance,
        attack_speed_mult,
        move_speed_mult,
        skill_cooldown_mult,
        ult_cooldown_mult,
        defence_penetration,
        magic_resistance_penetration,
        toughness,
        base_attack_damaged_reduce,
        magic_power_mult
    );
    // Mythical grants 35% more flat combat stats, +5 haste and a stronger proc budget.
    if upgraded {
        macro_rules! boost { ($($f:ident),*) => { $(stats.$f=(stats.$f as f64*1.35).round() as _;)* }; }
        boost!(
            attack,
            magic_power,
            hp,
            hp_regen,
            defence,
            magic_resistance,
            attack_speed_mult
        );
        stats.skill_cooldown_mult += 5;
        stats.ult_cooldown_mult += 5;
    }
    let mut price = if upgraded {
        (spec.price as f64 * 1.5 / 5.).round() as usize * 5
    } else {
        spec.price
    };
    if let Some(c) = config {
        macro_rules! over {($($f:ident),*) => { $(if let Some(n)=c.$f {stats.$f=n as _;})* };}
        over!(
            attack,
            magic_power,
            hp,
            hp_regen,
            defence,
            magic_resistance,
            vamp,
            crit_chance,
            attack_speed_mult,
            move_speed_mult,
            skill_cooldown_mult,
            ult_cooldown_mult,
            defence_penetration,
            magic_resistance_penetration,
            toughness,
            base_attack_damaged_reduce,
            magic_power_mult
        );
        price = c.price.unwrap_or(price);
    }
    MlbbItem {
        spec,
        upgraded,
        stats,
        price,
        procs: ProcQueue::new(),
        true_hits: Vec::new(),
        executions: Vec::new(),
        burns: BTreeMap::new(),
        ready: 0,
        secondary: 0,
        last_damage: 0,
        last_attack: 0,
        target: usize::MAX,
        count: 0,
        stacks: 0,
        permanent: 0,
        armed: 0,
        last_ult: 0,
        last_skills: (0, 0),
        revive_at: 0,
        last_tick: 0,
        combo: TimedStack::default(),
        target_stacks: BTreeMap::new(),
        accumulators: BTreeMap::new(),
        recoveries: Vec::new(),
        flask_windows: BTreeMap::new(),
        flask_cooldowns: BTreeMap::new(),
        last_aid_seq: 0,
        speed_until: 0,
        converted_base_crit: None,
        instance: rules::Instance::default(),
    }
}
impl MlbbItem {
    fn bonus_equipment_hp(ctx: &StableSim<'_>, id: usize) -> usize {
        rules::inventory(ctx, id).iter().map(|key| {
            if let Some(hp) = crate::patches::live::shown_flat(key, "hp") {
                return hp.max(0) as usize;
            }
            let base = rules::base(key);
            let hp = specs().iter().find(|spec| spec.key == base)
                .and_then(|spec| spec.stats.get("hp")).copied().unwrap_or(0).max(0);
            (hp as f64 * if key == base { 1. } else { 1.35 }).round() as usize
        }).sum()
    }

    fn update_classic_clock(&mut self, ctx: &mut StableSim<'_>, id: usize, preset: Preset) {
        let Preset::Clock { interval, hp, magic, stacks, bonus } = preset else { return; };
        let now = ctx.tick();
        let start = self.revive_at.checked_sub(1).unwrap_or(now);
        self.revive_at = start.saturating_add(1);
        let count = (now.saturating_sub(start) / ticks(interval).max(1)).min(stacks as usize);
        if self.permanent != count || rules::item_memory(ctx, id, &self.spec.key).is_none() {
            self.permanent = count;
            rules::keep_item_memory(ctx, id, &self.spec.key, count, self.revive_at);
        }
        self.buff(ctx, id, "time", BuffV1 {
            hp: self.power(f64::from(hp) * count as f64) as i32,
            magic_power: self.power(f64::from(magic) * count as f64) as i32,
            magic_power_mult: if count == stacks as usize { i32::from(bonus) } else { 0 },
            ..Default::default()
        }, 0.5);
    }

    fn owns_effects(&mut self, ctx: &StableSim<'_>, entity: usize) -> bool {
        // Native landing/shield hooks need this match even with zero heal power.
        crate::heal_shield_power::on_match_tick(ctx);
        self.ready = self
            .ready
            .max(rules::item_ready(ctx, entity, &self.spec.key));
        if let Some((permanent, revive_at)) = rules::item_memory(ctx, entity, &self.spec.key) {
            self.permanent = permanent;
            self.revive_at = revive_at;
        }
        rules::unique(ctx, entity, &self.key(), &mut self.instance)
    }
    fn update_immortality(&mut self, ctx: &mut StableSim<'_>, id: usize, available: bool) {
        let now = ctx.tick();
        if let Some(mut recovery) = rules::immortal_recovery(ctx, id) {
            if now < recovery.due {
                return;
            }
            let Some(((hp, max), level)) = ctx.get_entity(id).map(|e| (e.hp(), e.level())) else {
                return;
            };
            if hp == 0 || now > recovery.expires {
                rules::set_immortal_recovery(ctx, id, None);
                return;
            }
            if recovery.needs_hp {
                let restore = (max * self.tune("restore_hp", 16.) as usize / 100).max(1);
                recovery.needs_hp = hp < restore && !ctx.entity_set_hp(id, restore);
            }
            if recovery.needs_shield {
                let amount = self.power(self.tune("shield_base", 50.) + level as f64 * self.tune("shield_per_level", 25.));
                recovery.needs_shield = !rules::shield(ctx, id, amount, ticks(self.tune("shield_time", 3.)));
            }
            let incomplete = recovery.needs_hp || recovery.needs_shield;
            if now == recovery.due || !incomplete || now == recovery.expires {
                crate::tactics::heal_shield::immortality_status(|| format!(
                    "MLBB Immortality recovery: unit={id} tick={now} hp_ok={} shield_ok={}",
                    !recovery.needs_hp, !recovery.needs_shield
                ));
            }
            rules::set_immortal_recovery(ctx, id, (incomplete && now < recovery.expires).then_some(recovery));
            return;
        }
        if !available || now < self.ready {
            return;
        }
        if !rules::take_immortal_hit(ctx, id) {
            rules::note_protection(ctx, id, "immortality", now + 2);
            return;
        }

        self.start_cooldown(ctx, id, self.tune("cooldown", 210.));
        let duration = self.tune("recovery_time", 2.5);
        let banished = ctx.entity_banish(
            id, id, ticks(duration), "mlbb_immortality_hold", "mlbb_immortality_return",
        );
        let due = if banished { now + ticks(duration) } else { now };
        rules::set_immortal_recovery(ctx, id, Some(rules::ImmortalRecovery {
            due,
            expires: due + 2,
            needs_hp: true,
            needs_shield: true,
        }));
        crate::tactics::heal_shield::immortality_status(|| format!(
            "MLBB Immortality activation: unit={id} tick={now} stasis_accepted={banished}"
        ));
        if banished {
            rules::note_protection(ctx, id, "stasis", due);
            self.buff(ctx, id, "revive", BuffV1 {
                damaged_reduce: 100,
                ..Default::default()
            }, duration);
        } else {
            // Finish the save immediately if the host cannot enter stasis.
            self.update_immortality(ctx, id, available);
        }
    }
    fn start_cooldown(&mut self, ctx: &StableSim<'_>, entity: usize, seconds: f64) {
        self.ready = ctx.tick() + ticks(seconds);
        rules::keep_item_ready(ctx, entity, &self.spec.key, self.ready);
    }
    // Counts read the mechanic's own state, never infer stacks from bonus stats.
    fn icon_count(&self, now: usize, alive: bool) -> Option<usize> {
        match self.spec.key.as_str() {
            "clock_of_destiny" if matches!(passive_presets::equipped("clock_of_destiny"), Preset::Clock { .. }) => Some(self.permanent),
            "thunder_belt" if matches!(passive_presets::equipped("thunder_belt"), Preset::Thunder { .. }) => None,
            "thunder_belt" | "sky_piercer" => Some(self.permanent),
            "war_axe" | "clock_of_destiny" | "radiant_armor" |
            "brute_force_breastplate" | "corrosion_scythe" | "swift_crossbow" =>
                Some(if alive { self.combo.count(now) } else { 0 }),
            "warrior_boots" => Some(if alive &&
                (now.saturating_sub(self.last_attack) <= ticks(self.tune("out_of_combat", 5.)) ||
                 now.saturating_sub(self.last_damage) <= ticks(self.tune("out_of_combat", 5.))) { self.stacks.min(self.tune("stack_cap", 5.) as usize) } else { 0 }),
            _ => None,
        }
    }
    fn publish_count(&self, ctx: &StableSim<'_>, player: usize, alive: bool, event: bool) {
        if let Some(count) = self.icon_count(ctx.tick(), alive) {
            crate::ui::item_counts::on_match_tick(ctx);
            crate::ui::item_counts::note_mlbb(ctx, player, &self.key(), count as u64,
                !matches!(self.spec.key.as_str(),"thunder_belt"|"sky_piercer"),event);
        }
    }
    fn tune(&self, field: &str, default: f64) -> f64 {
        crate::passive_tuning::value(&self.spec.key, field, default)
    }

    fn power(&self, n: f64) -> usize {
        (n * if self.upgraded { 1.25 } else { 1. }).round().max(0.) as usize
    }
    fn name(&self, suffix: &str) -> String {
        if suffix == "lifebane" {
            "mlbb_shared_lifebane".to_string()
        } else {
            format!("mlbb_{}_{}", self.spec.key, suffix)
        }
    }
    fn buff(
        &self,
        ctx: &mut StableSim<'_>,
        id: usize,
        suffix: &str,
        mut buff: BuffV1,
        seconds: f64,
    ) {
        let name = self.name(suffix);
        buff.set_name(&name);
        buff.duration_kind = BuffDurationV1::Time.code();
        buff.duration_tick = ticks(seconds);
        if suffix == "lifebane" {
            rules::note_lifebane(ctx, id, ctx.tick() + ticks(seconds));
            rules::shared_buff(ctx, id, &name, buff, ticks(seconds));
        } else {
            refresh_buff(ctx, id, &name, &buff);
        }
    }
    fn slow(&self, ctx: &mut StableSim<'_>, target: usize, percent: i32, seconds: f64) {
        self.buff(
            ctx,
            target,
            "slow",
            BuffV1 {
                move_speed_mult: -percent,
                ..Default::default()
            },
            seconds,
        );
    }
    fn impulse(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        now: usize,
        repeats: usize,
        step: i32,
    ) {
        if !rules::active(ctx, caster, &self.key(), "impulse") {
            return;
        }
        let n = self.combo.gain(now, repeats, 5, ticks(3.), 0);
        rules::shared_buff(
            ctx,
            caster,
            "mlbb_shared_impulse",
            BuffV1 {
                attack_speed_mult: n as i32 * step,
                ..Default::default()
            },
            ticks(3.),
        );
    }
    fn carrier_ranged(ctx: &StableSim<'_>, caster: usize) -> bool {
        ctx.get_entity(caster)
            .and_then(|e| e.name())
            .and_then(|n| crate::champion_traits::traits(&n))
            .and_then(|t| t.ranged)
            .unwrap_or(false)
    }
    fn targets(ctx: &StableSim<'_>, owner: usize, center: usize, radius: u64) -> Vec<usize> {
        let Some((team, pos)) = ctx
            .get_entity(owner)
            .map(|e| (e.team(), ctx.get_entity(center).map(|e| e.pos())))
        else {
            return Vec::new();
        };
        let Some((x, y)) = pos else { return Vec::new() };
        (0..ctx.entity_count())
            .filter_map(|i| ctx.entity_at(i))
            .filter(|e| {
                e.team() != team
                    && e.is_alive()
                    && !e.is_tower()
                    && e.pos().0.abs_diff(x).saturating_pow(2)
                        + e.pos().1.abs_diff(y).saturating_pow(2)
                        <= radius.saturating_pow(2)
            })
            .map(|e| e.id())
            .collect()
    }
    fn refresh_malefic_range(&self, ctx: &mut StableSim<'_>, entity: usize) {
        let range = ctx.get_entity(entity)
            .and_then(|champion| champion.name())
            .and_then(|name| crate::champion_traits::traits(&name)?.attack_range);
        if let Some(range) = range {
            let bonus = ((range as u128 * self.tune("range_bonus", 12.) as u128 + 50) / 100) as usize;
            self.buff(ctx, entity, "range", BuffV1 { range: bonus, ..Default::default() }, 0.5);
        }
    }

    fn bloodthirsty_recovery(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        let now = ctx.tick();
        if now < self.ready {
            return;
        }
        let Some((hp, max)) = ctx.get_entity(entity).map(|e| e.hp()) else { return };
        if hp > 0 {
            self.start_cooldown(ctx, entity, self.tune("cooldown", 10.));
            self.recoveries.push((now + ticks(0.5), 6, self.power(max as f64 * self.tune("heal", 12.) / 600.), ticks(0.5)));
        }
    }

    fn haste_reset(ctx: &mut StableSim<'_>, entity: usize, percent: usize) {
        if let Some(cd) = ctx
            .get_entity(entity)
            .and_then(|e| e.cooldown(SkillSlotV1::Ult))
        {
            ctx.entity_reduce_cooldown(entity, SkillSlotV1::Ult, cd.remaining * percent / 100);
        }
    }
}
impl StableItem for MlbbItem {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }
    fn key(&self) -> String {
        format!(
            "{}{}",
            if self.upgraded { "mythical_" } else { "" },
            self.spec.key
        )
    }
    fn price(&self) -> usize {
        self.price
    }
    fn tier(&self) -> usize {
        if self.upgraded {
            4
        } else {
            self.spec.tier
        }
    }
    fn stat(&self) -> BuffV1 {
        let mut stats = self.stats;
        if rules::sustain_kind(&self.spec.key).is_some() {
            stats.vamp = 0;
        }
        stats
    }
    fn previous_tier(&self) -> Vec<String> {
        if self.upgraded {
            vec![self.spec.key.clone()]
        } else {
            self.spec.previous.clone()
        }
    }
    fn next_tier(&self) -> Vec<String> {
        if self.upgraded {
            vec![]
        } else {
            self.spec.next.clone()
        }
    }
    fn category(&self) -> ItemCategoryV1 {
        match self.spec.category.as_str() {
            "Mage" => ItemCategoryV1::Magic,
            "Tank" => {
                if self.stats.magic_resistance > self.stats.defence {
                    ItemCategoryV1::MagicResistance
                } else {
                    ItemCategoryV1::Defense
                }
            }
            "Support" => ItemCategoryV1::Support,
            _ => {
                if self.stats.attack_speed_mult > 0 {
                    ItemCategoryV1::AttackSpeed
                } else {
                    ItemCategoryV1::Ad
                }
            }
        }
    }
    fn tags(&self) -> Vec<ItemTagV1> {
        let mut out = Vec::new();
        if matches!(
            self.spec.key.as_str(),
            "sea_halberd" | "glowing_wand" | "dominance_ice" | "deadly_blade"
        ) {
            out.push(ItemTagV1::HealReduce);
        }
        for (yes, tag) in [
            (self.stats.attack > 0, ItemTagV1::Ad),
            (self.stats.magic_power > 0, ItemTagV1::Ap),
            (self.stats.hp > 0, ItemTagV1::Hp),
            (self.stats.defence > 0, ItemTagV1::Defense),
            (self.stats.magic_resistance > 0, ItemTagV1::MagicResistance),
            (self.stats.attack_speed_mult > 0, ItemTagV1::AttackSpeed),
            (self.stats.move_speed_mult > 0, ItemTagV1::MoveSpeed),
            (self.stats.vamp > 0 || rules::grants_sustain(&self.spec.key), ItemTagV1::Vamp),
            (
                self.stats.skill_cooldown_mult > 0,
                ItemTagV1::CooltimeReduce,
            ),
        ] {
            if yes {
                out.push(tag)
            }
        }
        out
    }
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // Keep item cooldowns and permanent stacks across respawn, reset transient combat state.
        if ctx.tick() < self.last_tick {
            self.ready = 0;
            self.secondary = 0;
            self.permanent = 0;
            self.flask_cooldowns.clear();
            self.last_aid_seq = 0;
        }
        self.procs.clear();
        self.true_hits.clear();
        self.executions.clear();
        self.burns.clear();
        self.stacks = 0;
        self.count = 0;
        self.armed = 0;
        self.revive_at = 0;
        self.target = usize::MAX;
        self.combo = TimedStack::default();
        self.target_stacks.clear();
        self.accumulators.clear();
        self.recoveries.clear();
        self.flask_windows.clear();
        self.speed_until = 0;
        self.last_damage = ctx.tick();
        self.last_attack = ctx.tick();
        self.last_ult = 0;
        self.last_skills = (0, 0);
        self.last_tick = ctx.tick();
        if self.spec.key == "thunder_belt" && matches!(passive_presets::equipped("thunder_belt"), Preset::Thunder { .. }) {
            if let Some((_, a, b, u)) = ctx.get_player(player).and_then(|p| p.cooldowns()) {
                self.last_skills = (a, b);
                self.last_ult = u;
            }
        }
        if self.spec.key == "malefic_gun" {
            self.secondary = ctx.tick();
            if let Some(entity) = ctx.get_player(player).and_then(|p| p.champion()).map(|c| c.id()) {
                self.refresh_malefic_range(ctx, entity);
            }
        }
    }
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        dt: DamageTypeV1,
        at: AttackTypeV1,
        crit: bool,
    ) {
        if !self.owns_effects(ctx, caster) {
            return;
        }
        // Typed item damage can drive non-recursive passives. Exclude producers
        // that would requeue their own payload; true item procs cannot echo War Axe.
        if at == AttackTypeV1::Item
            && !matches!(
                self.spec.key.as_str(),
                "sea_halberd"
                    | "glowing_wand"
                    | "genius_wand"
                    | "clock_of_destiny"
                    | "concentrated_energy"
                    | "hunter_strike"
                    | "blade_of_despair"
                    | "brute_force_breastplate"
            )
            && !(self.spec.key == "war_axe" && dt != DamageTypeV1::Fixed)
        {
            return;
        }
        let Some((hp, max, champ, tower, ts)) = ctx.get_entity(target).map(|e| {
            let (h, m) = e.hp();
            (h, m, e.is_champion(), e.is_tower(), e.stat())
        }) else {
            return;
        };
        if tower {
            return;
        }
        let Some((cs, level)) = ctx.get_entity(caster).map(|e| (e.stat(), e.level())) else {
            return;
        };
        let now = ctx.tick();
        let basic = at == AttackTypeV1::BaseAttack;
        let skill = at == AttackTypeV1::Skill;
        let preset = passive_presets::equipped(&self.spec.key);
        let event = rules::attack_event(ctx, caster, target, *damage, basic, crit);
        let repeats = event.repeats;
        let idle = now.saturating_sub(self.last_attack) > ticks(5.)
            && now.saturating_sub(self.last_damage) > ticks(5.);
        if champ {
            self.last_attack = now;
        }
        let mut physical = 0;
        let mut magic = 0;
        let mut fixed = 0;
        match self.spec.key.as_str() {
            "raptor_machete" if basic && crate::role_gear::jungle_item_active(ctx,caster,&self.key()) => {
                if now>=self.ready {
                    self.start_cooldown(ctx,caster,10.);
                    self.buff(ctx,target,"breakout",BuffV1{defence:-(self.power(7.) as i32),..Default::default()},3.);
                    self.slow(ctx,target,30,1.5);
                } else {
                    self.ready=self.ready.saturating_sub(ticks(1.));
                    rules::keep_item_ready(ctx,caster,&self.spec.key,self.ready);
                }
            }

            "berserkers_fury" if basic && crit => {
                fixed = self.power(*damage as f64 * self.tune("true_damage", 12.) / 100.);
                *damage += *damage * self.tune("crit_damage", 30.) as usize / 100;
            }
            "blade_of_despair" if champ && hp * 2 < max => {
                *damage += *damage * self.power(self.tune("damage_bonus", 25.)) / 100;
                self.buff(ctx, caster, "despair", BuffV1::default(), 2.);
            }
            "blade_of_the_heptaseas" if basic && idle => {
                physical = self.power(self.tune("base_damage", 56.) + cs.attack as f64 * self.tune("attack_ratio", 40.) / 100.);
                self.slow(ctx, target, self.tune("slow", 40.) as i32, self.tune("slow_time", 1.5));
            }
            "corrosion_scythe" if basic => {
                physical = self.power(self.tune("base_damage", 28.)) * repeats;
                let n = self.target_stacks.entry(target).or_default().gain(
                    now,
                    repeats,
                    5,
                    ticks(1.5),
                    0,
                );
                let ranged = Self::carrier_ranged(ctx, caster);
                self.slow(ctx, target, n as i32 * if ranged { 4 } else { 8 }, 1.5);
                self.impulse(ctx, caster, now, repeats, self.tune("impulse_speed", 6.) as i32);
            }
            "demon_hunter_sword" if basic => {
                self.procs.push_physical(
                    ctx,
                    target,
                    self.power(hp as f64 * self.tune("hp_damage", 8.) / 100.)
                        .min(if champ { usize::MAX } else { self.tune("monster_cap", 60.) as usize }),
                );
                for _ in 1..repeats {
                    self.procs.push_current_hp_physical(
                        ctx,
                        target,
                        self.tune("hp_damage", 8.) * if self.upgraded { 1.25 } else { 1. },
                        if champ { usize::MAX } else { self.tune("monster_cap", 60.) as usize },
                    );
                }
                ctx.heal(
                    caster,
                    caster,
                    self.power(self.tune("heal_base", 10.) + self.tune("heal_per_level", 4.) * level as f64) * repeats * if champ { 2 } else { 1 } / 2,
                );
            }
            "azure_blade" if basic && self.armed >= now && self.armed > 0 && now >= self.ready
                    && (self.spec.key != "starlium_scythe"
                        || rules::active(ctx, caster, &self.key(), "calamity")) => {
                self.armed = 0;
                self.start_cooldown(ctx, caster, self.tune("cooldown", 1.5));
                fixed = self.power(self.tune("base_damage", 18.));
            }
            "swift_crossbow" if basic => {
                if cs.magic_power > cs.attack {
                    magic = self.power(self.tune("base_damage", 14.)) * repeats;
                } else {
                    physical = self.power(self.tune("base_damage", 14.)) * repeats;
                }
                self.impulse(ctx, caster, now, repeats, self.tune("impulse_speed", 3.) as i32);
            }
            "endless_battle" | "starlium_scythe"
                if basic && self.armed >= now && self.armed > 0 && now >= self.ready =>
            {
                self.armed = 0;
                self.start_cooldown(ctx, caster, self.tune("cooldown", 1.5));
                fixed = self.power(if self.spec.key == "endless_battle" {
                    cs.attack as f64 * self.tune("attack_ratio", 60.) / 100.
                } else {
                    self.tune("base_damage", 30.) + cs.magic_power as f64 * self.tune("magic_ratio", 60.) / 100.
                });
                if self.spec.key == "endless_battle" {
                    ctx.heal(caster, caster, self.power(self.tune("heal_base", 28.) + cs.attack as f64 * self.tune("heal_ratio", 40.) / 100.));
                } else {
                    self.slow(ctx, target, self.tune("slow", 15.) as i32, self.tune("slow_time", 1.5));
                }
            }
            "calamity_reaper"
                if basic && self.armed >= now && self.armed > 0
                    && now >= self.ready && rules::active(ctx, caster, &self.key(), "calamity") =>
            {
                self.armed = 0;
                self.start_cooldown(ctx, caster, self.tune("cooldown", 1.5));
                fixed = self.power(cs.magic_power as f64 * self.tune("magic_ratio", 75.) / 100.);
                self.buff(ctx, caster, "calamity", BuffV1 {
                    move_speed_mult: self.tune("movement_speed", 10.) as i32,
                    ..Default::default()
                }, self.tune("speed_time", 1.));
            }
            "shadow_twinblades" if basic && champ && idle => {
                magic = self.power(self.tune("base_damage", 45.) + cs.magic_power as f64 * self.tune("magic_ratio", 50.) / 100.);
                self.slow(ctx, target, self.tune("slow", 50.) as i32, self.tune("slow_time", 1.5));
            }
            "ice_force" if basic && champ && now >= self.ready => {
                self.start_cooldown(ctx, caster, self.tune("cooldown", 3.));
                self.buff(ctx, target, "frost_strike", BuffV1 {
                    move_speed_mult: -(self.tune("slow", 25.) as i32),
                    attack_speed_mult: -(self.tune("attack_slow", 15.) as i32),
                    ..Default::default()
                }, self.tune("slow_time", 1.5));
            }
            "golden_staff" if basic && !crit && repeats == 3 => {
                self.buff(
                    ctx,
                    caster,
                    "strike",
                    BuffV1 {
                        attack_speed_mult: self.tune("empowered_speed", 80.) as i32,
                        ..Default::default()
                    },
                    self.tune("speed_time", 0.5),
                );
            }
            "haas_claws" | "scarlet_phantom"
                if basic && crit && rules::active(ctx, caster, &self.key(), "frenzy") =>
            {
                let scarlet = self.spec.key == "scarlet_phantom";
                rules::shared_buff(ctx, caster, "mlbb_shared_frenzy", BuffV1 {
                    attack_speed_mult: self.tune("frenzy_speed", if scarlet { 30. } else { 20. }) as i32,
                    crit_chance: if scarlet { self.tune("frenzy_crit", 5.) as i32 } else { 0 },
                    ..Default::default()
                }, ticks(self.tune("frenzy_time", 2.)));
            }
            "hunter_strike" if champ || is_monster_target(ctx, target) => {
                if self.target != target {
                    self.target = target;
                    self.count = 0;
                }
                self.count += 1;
                if self.count >= self.tune("hits", 5.) as usize && now >= self.ready {
                    self.count = 0;
                    self.start_cooldown(ctx, caster, self.tune("cooldown", 8.));
                    self.speed_until = now + ticks(self.tune("duration", 3.));
                }
            }
            "malefic_gun" if basic => {
                self.buff(
                    ctx,
                    caster,
                    "energy",
                    BuffV1 {
                        move_speed_mult: self.tune("movement_speed", 10.) as i32,
                        ..Default::default()
                    },
                    self.tune("speed_time", 0.5),
                );
            }
            "deadly_blade" if basic && champ => {
                self.buff(
                    ctx,
                    target,
                    "lifebane",
                    BuffV1 {
                        heal_reduce: 40,
                        ..Default::default()
                    },
                    self.tune("lifebane_time", 1.5),
                );
            }
            "sea_halberd" | "glowing_wand" => {
                self.buff(
                    ctx,
                    target,
                    "lifebane",
                    BuffV1 {
                        heal_reduce: 40,
                        ..Default::default()
                    },
                    self.tune("lifebane_time", 3.),
                );
                if self.spec.key == "sea_halberd" && ts.hp > cs.hp {
                    *damage += *damage * self.tune("damage_bonus", 8.) as usize / 100;
                }
                if self.spec.key == "glowing_wand"
                    && dt == DamageTypeV1::Ap
                    && at != AttackTypeV1::Item
                {
                    self.burns
                        .insert(target, (now + ticks(self.tune("burn_time", 3.)), now + ticks(1.)));
                }
            }
            "war_axe" => {
                let cap = self.tune("stack_cap", 6.) as usize;
                let duration = self.tune("stack_time", 4.);
                self.combo.gain(now, 1, cap, ticks(duration), ticks(1.));
                let n = self.combo.count(now);
                self.buff(
                    ctx,
                    caster,
                    "fighting",
                    BuffV1 {
                        attack: n as i32 * self.tune("attack_per_stack", 4.) as i32,
                        ..Default::default()
                    },
                    duration,
                );
                if n == cap {
                    fixed = self.power(event.raw as f64 * self.tune("true_damage", 10.) / 100.) * if basic { repeats } else { 1 };
                }
            }
            "windtalker" | "legacy_windtalker" if basic && rules::active(ctx, caster, &self.key(), "typhoon") => {
                let legacy = self.spec.key == "legacy_windtalker";
                let shared = rules::item_ready(ctx, caster, "windtalker");
                self.ready = if legacy { shared } else { shared.saturating_sub(ticks(self.tune("hit_reduction", 0.2))) };
                if legacy && self.permanent < 5 {
                    self.permanent += 1;
                    return;
                }
                if now >= self.ready {
                    self.ready = now + ticks(self.tune("cooldown", 4.));
                    if legacy {
                        self.permanent = 0;
                        self.ready = now;
                        self.buff(ctx, caster, "activate", BuffV1 {
                            move_speed_mult: self.tune("movement_speed", 10.) as i32,
                            ..Default::default()
                        }, self.tune("speed_time", 1.));
                    }
                    for id in Self::targets(ctx, caster, target, 60000)
                        .into_iter()
                        .take(self.tune("targets", 3.) as usize)
                    {
                        self.procs
                            .push_magic(ctx, id, self.power(self.tune("base_damage", 50.) + level as f64 * self.tune("level_damage", 5.)));
                    }
                }
                rules::keep_item_ready(ctx, caster, "windtalker", self.ready);
            }
            "clock_of_destiny" if preset == Preset::Default && dt == DamageTypeV1::Ap && champ => {
                let duration = self.tune("stack_time", 5.);
                let n = self.combo.gain(now, 1, self.tune("stack_cap", 6.) as usize, ticks(duration), ticks(self.tune("stack_gate", 0.4)));
                let defence = (n as f64 * (self.tune("resist_base", 4.5) + level.min(15) as f64 * self.tune("resist_level", 0.5))).round() as i32;
                self.buff(
                    ctx,
                    caster,
                    "destiny",
                    BuffV1 {
                        defence,
                        magic_resistance: defence,
                        ..Default::default()
                    },
                    duration,
                );
            }
            "concentrated_energy" if dt == DamageTypeV1::Ap => {
                let cap = self.tune("stack_cap", 6.) as usize;
                let duration = self.tune("stack_time", 5.);
                self.combo.gain(now, 1, cap, ticks(duration), ticks(self.tune("stack_gate", 0.4)));
                let n = self.combo.count(now);
                self.buff(
                    ctx,
                    caster,
                    "recharge",
                    BuffV1 {
                        magic_power: n as i32 * self.tune("magic_per_stack", 3.) as i32,
                        ..Default::default()
                    },
                    duration,
                );
                if n == cap {
                    *damage += *damage * self.tune("damage_bonus", 12.) as usize / 100;
                }
            }
            "feather_of_heaven" if basic => {
                magic = self.power(self.tune("base_damage", 18.) + cs.magic_power as f64 * self.tune("magic_ratio", 30.) / 100.) * repeats;
                self.impulse(ctx, caster, now, repeats, self.tune("impulse_speed", 6.) as i32);
            }
            "genius_wand" if dt == DamageTypeV1::Ap && champ => {
                let duration = self.tune("duration", 2.);
                let cap = self.tune("cap", 3.) as usize;
                let low = self.tune("level_one", 2.) as i32;
                let high = self.tune("level_max", 7.) as i32;
                let reduction = low + (high - low) * level.min(15) as i32 / 15;
                let n = self
                    .target_stacks
                    .entry(target)
                    .or_default()
                    .gain(now, 1, cap, ticks(duration), 0);
                self.buff(
                    ctx,
                    target,
                    "magic",
                    BuffV1 {
                        magic_resistance: -(n as i32 * reduction),
                        ..Default::default()
                    },
                    duration,
                );
            }
            "ice_queen_wand" if skill && champ => {
                let cap = self.tune("slow_cap", 3.) as usize;
                let duration = self.tune("slow_time", 2.);
                let gate = self.tune("stack_gate", 0.4);
                let n = self.target_stacks.entry(target).or_default()
                    .gain(now, 1, cap, ticks(duration), ticks(gate));
                self.slow(ctx, target, n as i32 * self.tune("slow_per_stack", 10.) as i32, duration);
            }
            "lightning_truncheon" if skill && now >= self.ready => {
                self.start_cooldown(ctx, caster, self.tune("cooldown", 6.));
                for id in Self::targets(ctx, caster, target, 60000) {
                    self.procs
                        .push_magic(ctx, id, self.power(self.tune("base_damage", 85.) + cs.magic_power as f64 * self.tune("magic_ratio", 85.) / 100.));
                }
            }
            "wishing_lantern" if dt == DamageTypeV1::Ap && champ => {
                let threshold = (self.tune("damage_threshold", 315.) as usize).max(1);
                let hp_damage = self.tune("hp_damage", 8.) * if self.upgraded { 1.25 } else { 1. };
                let sum = self.accumulators.entry(target).or_default();
                *sum = sum.saturating_add(event.raw);
                let procs = (*sum / threshold).min(32);
                *sum = sum.saturating_sub(procs * threshold);
                for _ in 0..procs {
                    self.procs.push_current_hp_magic(
                        ctx,
                        target,
                        hp_damage,
                    );
                }
            }
            "brute_force_breastplate" => {
                let cap = self.tune("stack_cap", 6.) as usize;
                let duration = self.tune("stack_time", 4.);
                let n = self.combo.gain(now, 1, cap, ticks(duration), ticks(1.)) as i32;
                let magic = cs.magic_power > cs.attack;
                self.buff(
                    ctx,
                    caster,
                    "brute",
                    BuffV1 {
                        attack: if magic { 0 } else { n * self.tune("attack_per_stack", 2.) as i32 },
                        magic_power: if magic { n * self.tune("magic_per_stack", 3.) as i32 } else { 0 },
                        move_speed_mult: n * self.tune("speed_per_stack", 2.) as i32,
                        toughness: if n == cap as i32 { self.tune("max_stack_tenacity", 25.) as usize } else { 0 },
                        ..Default::default()
                    },
                    duration,
                );
            }
            "twilight_armor" if basic && now >= self.ready => {
                if let Preset::TwilightOnHit { interval, damage, hp_percent } = preset {
                    self.start_cooldown(ctx, caster, interval);
                    magic = self.power(f64::from(damage) + Self::bonus_equipment_hp(ctx, caster) as f64 * hp_percent / 100.);
                }
            }
            "thunder_belt" if basic && now >= self.ready => {
                if let Preset::Thunder { damage, hp_percent, slow, max_slow, slow_scale, duration, cooldown, .. } = preset {
                    if self.armed != 0 && now <= self.armed {
                        self.armed = 0;
                        self.start_cooldown(ctx, caster, cooldown);
                        let bonus_hp = Self::bonus_equipment_hp(ctx, caster) as f64;
                        let amount = self.power(f64::from(damage) + bonus_hp * hp_percent / 100.);
                        let slow = (f64::from(slow) + bonus_hp * slow_scale / 100.)
                            .min(f64::from(max_slow)).round() as i32;
                        self.true_hits.push((target, amount));
                        self.slow(ctx, target, slow, duration);
                        for nearby in Self::targets(ctx, caster, target, crate::sized_range(ctx, target, 40)) {
                            if nearby != target {
                                self.true_hits.push((nearby, amount));
                                self.slow(ctx, nearby, slow, duration);
                            }
                        }
                    }
                } else {
                    self.start_cooldown(ctx, caster, self.tune("cooldown", 4.));
                    fixed = self.power(self.tune("base_damage", 18.) + (cs.defence + cs.magic_resistance) as f64 * self.tune("resist_ratio", 35.) / 100.);
                    self.slow(ctx, target, self.tune("slow", 90.) as i32, self.tune("slow_time", 0.3));
                    if champ {
                        self.permanent += 1;
                        rules::keep_item_memory(ctx, caster, &self.spec.key, self.permanent, 0);
                    }
                }
            }
            "sky_piercer" if champ => {
                self.executions.push((target, self.tune("base_threshold", 4.) as usize * 10 + self.permanent.min(self.tune("stack_cap", 80.) as usize)));
            }
            _ => {}
        }
        self.procs
            .on_hit_physical(ctx, target, damage, dt, crit, physical);
        self.procs
            .on_hit_magic(ctx, target, damage, dt, crit, magic);
        if fixed > 0 {
            self.true_hits.push((target, fixed));
        }
    }
    // Flask eligibility comes from actual heal / attributed native shield grants,
    // not generic on_skill_hit (which also reports arbitrary buffs).
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        dt: DamageTypeV1,
        at: AttackTypeV1,
        _crit: bool,
    ) {
        if !self.owns_effects(ctx, entity) {
            return;
        }
        if self.spec.key == "warrior_boots" && at == AttackTypeV1::BaseAttack {
            self.stacks = (self.stacks + 1).min(self.tune("stack_cap", 5.) as usize);
        }
        self.last_damage = ctx.tick();
        let Some((hp, max, stat, level)) = ctx.get_entity(entity).map(|e| {
            let (h, m) = e.hp();
            (h, m, e.stat(), e.level())
        }) else {
            return;
        };
        match self.spec.key.as_str() {
            "beast_killer" if at!=AttackTypeV1::Item && hp>0 && crate::role_gear::jungle_item_active(ctx,entity,&self.key()) => {
                self.count=self.count.saturating_add(damage);
                if self.count>=max/10 && ctx.tick()>=self.ready {
                    self.count=0;self.start_cooldown(ctx,entity,2.);
                    let amount=self.power(35.+3.*level as f64);
                    for target in Self::targets(ctx,entity,entity,crate::sized_range(ctx,entity,50)) {
                        ctx.deal_damage_typed(entity,target,amount,DamageTypeV1::Ap,AttackTypeV1::Item);
                        self.slow(ctx,target,30,2.);
                    }
                }
            }

            "antique_cuirass" | "dreadnaught_armor"
                if at == AttackTypeV1::Skill
                    && rules::active(ctx, entity, &self.key(), "deter") =>
            {
                rules::note_deter(
                    ctx,
                    attacker,
                    if self.spec.key == "antique_cuirass" {
                        self.tune("damage_reduction", 8.) as usize
                    } else {
                        self.tune("damage_reduction", 4.) as usize
                    },
                );
                self.buff(ctx, attacker, "deter", BuffV1::default(), 2.);
            }
            "blade_armor" if at == AttackTypeV1::BaseAttack => {
                self.procs.push_physical(
                    ctx,
                    attacker,
                    self.power(damage as f64 * (self.tune("reflect", 30.) + stat.defence as f64 * self.tune("armor_ratio", 0.2)) / 100.),
                );
                self.slow(ctx, attacker, self.tune("slow", 15.) as i32, self.tune("slow_time", 1.));
            }
            "chastise_pauldron" => {
                let name = "mlbb_shared_chastise";
                if let Some(e) = ctx.get_entity(attacker) {
                    let old = (0..e.buff_count())
                        .filter_map(|i| e.buff_at(i))
                        .find(|b| b.name() == name)
                        .map_or(0, |b| b.attack_speed_mult);
                    let without_this = (e.attack_speed_mult() as i32 - old).max(1);
                    refresh_buff(
                        ctx,
                        attacker,
                        name,
                        &BuffV1 {
                            attack_speed_mult: -(without_this * self.tune("attack_slow", 20.) as i32 / 100),
                            ..BuffV1::timed(name, ticks(self.tune("slow_time", 2.)))
                        },
                    );
                }
            }
            "radiant_armor" if dt == DamageTypeV1::Ap => {
                let duration = self.tune("stack_time", 5.);
                let n = self.combo.gain(ctx.tick(), 1, self.tune("stack_cap", 6.) as usize, ticks(duration), ticks(self.tune("stack_gate", 0.4)));
                self.buff(
                    ctx,
                    entity,
                    "holy",
                    BuffV1 {
                        magic_resistance: n as i32 * (3 + level as i32 / 2),
                        ..Default::default()
                    },
                    duration,
                );
            }
            "twilight_armor" if passive_presets::equipped("twilight_armor") == Preset::Default && damage > self.tune("recovery_threshold", 175.) as usize && hp > 0 => {
                ctx.heal(
                    entity,
                    entity,
                    self.power((damage - self.tune("recovery_threshold", 175.) as usize) as f64 * (0.2 + max as f64 * 0.0002).min(0.8)),
                );
            }
            _ => {}
        }
        if hp == 0 || ctx.tick() < self.ready {
            return;
        }
        let low = hp * 100
            <= max
                * match self.spec.key.as_str() {
                    "queens_wings" => self.tune("threshold", 40.) as usize,
                    "chastise_pauldron" => self.tune("threshold", 30.) as usize,
                    "twilight_armor" => self.tune("trigger_hp", 30.) as usize,
                    _ => 30,
                };
        if !low {
            return;
        }
        match self.spec.key.as_str() {
            "magic_blade"
                if rules::active(ctx, entity, &self.key(), "lifeline")
                    && rules::claim_cooldown(ctx, entity, "lifeline", ticks(self.tune("cooldown", 60.))) =>
            {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 60.));
                rules::shield(ctx, entity, self.power(level as f64 * self.tune("shield_per_level", 21.)), ticks(self.tune("shield_time", 3.)));
                self.buff(
                    ctx,
                    entity,
                    "lifeline",
                    BuffV1 {
                        move_speed_mult: self.tune("movement_speed", 25.) as i32,
                        ..Default::default()
                    },
                    self.tune("shield_time", 3.),
                );
            }
            "rose_gold_meteor"
                if rules::active(ctx, entity, &self.key(), "lifeline")
                    && rules::claim_cooldown(ctx, entity, "lifeline", ticks(self.tune("cooldown", 60.))) =>
            {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 60.));
                rules::shield(ctx, entity, self.power(level as f64 * self.tune("shield_per_level", 42.)), ticks(self.tune("shield_time", 3.)));
                self.buff(
                    ctx,
                    entity,
                    "lifeline",
                    BuffV1 {
                        move_speed_mult: self.tune("movement_speed", 25.) as i32,
                        ..Default::default()
                    },
                    self.tune("shield_time", 3.),
                );
            }
            "queens_wings" => {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 60.));
                self.buff(
                    ctx,
                    entity,
                    "demonize",
                    BuffV1 {
                        damaged_reduce: self.tune("damage_reduction", 30.) as usize,
                        ..Default::default()
                    },
                    self.tune("duration", 3.),
                );
                for slot in [SkillSlotV1::Skill, SkillSlotV1::Skill2] {
                    ctx.entity_reduce_cooldown(entity, slot, ticks(self.tune("skill_reduction", 2.)));
                }
            }
            "wind_of_nature" => {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 90.));
                let duration = if ctx
                    .get_entity(entity)
                    .and_then(|e| e.name())
                    .and_then(|n| crate::champion_traits::traits(&n))
                    .is_some_and(|t| t.class == Some(crate::champion_traits::Class::Range))
                {
                    self.tune("marksman_duration", 2.)
                } else {
                    self.tune("other_duration", 1.)
                };
                rules::note_protection(ctx, entity, "physical", ctx.tick() + ticks(duration));
                self.buff(ctx, entity, "wind_chant", BuffV1::default(), duration);
            }
            "chastise_pauldron" => {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 60.));
                self.recoveries
                    .push((ctx.tick() + ticks(0.5), 4, self.power(max as f64 * self.tune("heal", 20.) / 400.), ticks(0.5)));
            }
            "twilight_armor" if passive_presets::equipped("twilight_armor") == Preset::Default => {
                self.start_cooldown(ctx, entity, self.tune("cooldown", 60.));
                ctx.heal(entity, entity, max * self.tune("low_hp_heal", 20.) as usize / 100);
            }
            _ => {}
        }
    }
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng: u64, player: usize) {
        crate::role_gear::item_tick(ctx);
        let Some((id, hp, max, stat, level, targetable)) =
            ctx.get_player(player).and_then(|p| p.champion()).map(|e| {
                let (h, m) = e.hp();
                (e.id(), h, m, e.stat(), e.level(), e.is_targetable())
            })
        else {
            return;
        };
        let now = ctx.tick();
        self.last_tick = now;
        if !self.owns_effects(ctx, id) {
            self.procs.clear();
            self.true_hits.clear();
            self.executions.clear();
            self.burns.clear();
            return;
        }
        if hp > 0 && self.spec.key == "immortality" {
            self.update_immortality(ctx, id, crate::tactics::heal_shield::landing_installed());
        }
        let preset = passive_presets::equipped(&self.spec.key);
        if hp > 0 {
            self.update_classic_clock(ctx, id, preset);
            if matches!(preset, Preset::Twilight { .. } | Preset::Defiance { .. }) {
                rules::note_twilight(ctx, id, max, preset);
            }
            if let Preset::Defiance { hp_percent, .. } = preset {
                if rules::take_defiance_pulse(ctx, id) {
                    let amount = self.power(Self::bonus_equipment_hp(ctx, id) as f64 * hp_percent / 100.);
                    for target in Self::targets(ctx, id, id, crate::sized_range(ctx, id, 40)) {
                        self.true_hits.push((target, amount));
                    }
                }
            }
        }
        if hp > 0 && self.spec.key == "malefic_gun" && now >= self.secondary {
            self.secondary = now + ticks(0.25);
            self.refresh_malefic_range(ctx, id);
        }
        if hp > 0 && self.spec.key == "valkyries_shield" {
            rules::note_magic_guard(ctx, id, self.tune("reduction", 25.) as usize,
                ticks(self.tune("duration", 3.)), ticks(self.tune("rearm", 5.)));
        }
        if hp > 0 && self.spec.key == "guardian_helmet" {
            rules::note_protection(ctx, id, "guardian", now + 2);
        }
        if self.spec.key == "oracle" {
            rules::note_oracle(ctx, id, self.tune("received_bonus", 25.) as usize);
        }
        if self.spec.key == "flask_of_the_oasis" && rules::active(ctx, id, &self.key(), "blessing")
        {
            crate::heal_shield_power::note(ctx, player, self.tune("heal_shield_power", 12.) * if self.upgraded { 1.25 } else { 1. });
            let (cursor, events) = rules::aid_events(ctx, id, self.last_aid_seq);
            self.last_aid_seq = cursor;
            for (target, at) in events {
                if let Some(recipient) = ctx.get_entity(target) {
                    if recipient.is_champion()
                        && recipient.team()
                            == ctx.get_entity(id).map(|e| e.team()).unwrap_or(usize::MAX)
                    {
                        self.flask_windows.insert(target, (at + ticks(self.tune("window", 3.)), level));
                    }
                }
            }
            self.flask_windows.retain(|&target, (until, _)| {
                *until >= now && ctx.get_entity(target).is_some_and(|e| e.is_alive())
            });
            let due: Vec<_> = self
                .flask_windows
                .iter()
                .filter_map(|(&target, &(_, cast_level))| {
                    let (h, m) = ctx.get_entity(target)?.hp();
                    (h > 0
                        && h * 100 < m * self.tune("ally_hp", 35.) as usize
                        && now >= *self.flask_cooldowns.get(&target).unwrap_or(&0))
                    .then_some((target, cast_level))
                })
                .collect();
            for (target, cast_level) in due {
                self.flask_windows.remove(&target);
                if !rules::claim_cooldown(ctx, target, "blessing", ticks(self.tune("cooldown", 60.))) {
                    continue;
                }
                self.flask_cooldowns.insert(target, now + ticks(self.tune("cooldown", 60.)));
                rules::shield(ctx, target, self.power(cast_level as f64 * self.tune("shield_per_level", 35.)), ticks(self.tune("shield_time", 3.)));
                for slot in [SkillSlotV1::Skill, SkillSlotV1::Skill2] {
                    ctx.entity_reduce_cooldown(id, slot, ticks(self.tune("skill_reduction", 2.)));
                }
            }
        }
        // Progress recovery independently of the half-second aura update gate.
        if hp == 0 {
            self.recoveries.clear();
        }
        let mut due = Vec::new();
        self.recoveries.retain_mut(|(next, left, amount, interval)| {
            if now >= *next && *left > 0 {
                due.push(*amount);
                *left -= 1;
                *next += *interval;
            }
            *left > 0
        });
        for amount in due {
            crate::heal_shield_power::unraised(|| ctx.heal(id, id, amount));
        }
        if hp > 0 && now >= self.ready && preset == Preset::Default
            && matches!(self.spec.key.as_str(), "clock_of_destiny" | "elegant_gem")
            && hp.saturating_mul(100) < max.saturating_mul(self.tune("gift_threshold", 50.) as usize)
            && rules::active(ctx, id, &self.key(), "gift")
        {
            let cooldown = self.tune("gift_cooldown", 60.);
            if rules::claim_cooldown(ctx, id, "gift", ticks(cooldown)) {
                let interval = self.tune("gift_duration", 3.) / 6.;
                self.start_cooldown(ctx, id, cooldown);
                let amount = self.power(max as f64 * self.tune("gift_recovery", 15.) / 600.);
                self.recoveries.push((now + ticks(interval), 6, amount, ticks(interval)));
            }
        }
        self.procs.update(ctx, player);
        for (target, amount) in std::mem::take(&mut self.true_hits) {
            if ctx.get_entity(target).is_some_and(|e| e.is_alive()) {
                ctx.deal_damage_typed(id, target, amount, DamageTypeV1::Fixed, AttackTypeV1::Item);
            }
        }
        for (target, threshold) in std::mem::take(&mut self.executions) {
            if let Some((h, m)) = ctx
                .get_entity(target)
                .filter(|e| e.is_alive())
                .map(|e| e.hp())
            {
                if h > 0 && h <= m * threshold / 1000 {
                    ctx.deal_damage_typed(id, target, h, DamageTypeV1::Fixed, AttackTypeV1::Item);
                }
            }
        }
        self.publish_count(ctx, player, hp > 0, false);
        if hp == 0 {
            return;
        }
        if now.saturating_sub(self.last_attack) > ticks(5.)
            && now.saturating_sub(self.last_damage) > ticks(5.)
        {
            self.stacks = 0;
            self.count = 0;
        }
        // Detect action cooldown starts. This covers attacks that miss and ally-targeted casts.
        if let Some((_, a, b, u)) = ctx.get_player(player).and_then(|p| p.cooldowns()) {
            let transition =
                a > self.last_skills.0 + 1 || b > self.last_skills.1 + 1 || u > self.last_ult + 1;
            self.last_skills = (a, b);
            if let Preset::Thunder { window, .. } = preset {
                if transition { self.armed = now + ticks(window); }
            }
            if transition
                && matches!(
                    self.spec.key.as_str(),
                    "endless_battle" | "starlium_scythe" | "azure_blade" | "calamity_reaper"
                )
            {
                self.armed = now + ticks(self.tune("cast_window", 3.));
            }
            if u > self.last_ult + 1 && self.spec.key == "great_dragon_spear" && now >= self.ready {
                self.start_cooldown(ctx, id, self.tune("cooldown", 15.));
                self.buff(
                    ctx,
                    id,
                    "supreme",
                    BuffV1 {
                        move_speed_mult: self.tune("movement_speed", 30.) as i32,
                        ..Default::default()
                    },
                    self.tune("duration", 7.5),
                );
            }
            self.last_ult = u;
        }
        if self.spec.key == "winter_crown"
            && !rules::immortal_recovery_pending(ctx,id)
            && hp * 100 <= max * self.tune("threshold", 30.) as usize
            && targetable
            && now >= self.ready
        {
            self.start_cooldown(ctx, id, self.tune("cooldown", 100.));
            let duration = self.tune("stasis_time", 2.);
            rules::note_protection(ctx, id, "stasis", now + ticks(duration));
            ctx.entity_banish(
                id, id, ticks(duration),
                "mlbb_winter_frozen", "mlbb_winter_break",
            );
            self.buff(
                ctx,
                id,
                "frozen",
                BuffV1 {
                    damaged_reduce: 100,
                    ..Default::default()
                },
                duration,
            );
        }
        if self.spec.key == "golden_staff" {
            let keys = rules::inventory(ctx, id);
            let gear = keys
                .iter()
                .filter_map(|k| specs().iter().find(|s| s.key == rules::base(k)))
                .map(|s| s.stats.get("crit_chance").copied().unwrap_or(0).max(0) as usize)
                .sum::<usize>();
            let baseline = *self
                .converted_base_crit
                .get_or_insert(stat.crit_chance.saturating_sub(gear));
            self.buff(
                ctx,
                id,
                "conversion",
                BuffV1 {
                    crit_chance: -10000,
                    attack_speed_mult: (gear + baseline).min(1000) as i32,
                    ..Default::default()
                },
                0.1,
            );
        }
        if self.spec.key == "hunter_strike" && now < self.speed_until {
            let speed = self.tune("movement_speed", 50.) as usize * self.speed_until.saturating_sub(now)
                / ticks(self.tune("duration", 3.)).max(1);
            self.buff(
                ctx,
                id,
                "retribution",
                BuffV1 {
                    move_speed_mult: speed as i32,
                    ..Default::default()
                },
                0.1,
            );
        }
        self.target_stacks.retain(|&target, s| {
            s.expires > now && ctx.get_entity(target).is_some_and(|e| e.is_alive())
        });
        self.accumulators
            .retain(|&target, _| ctx.get_entity(target).is_some_and(|e| e.is_alive()));
        if self.spec.key == "blood_wings" {
            blood::update(ctx, id, self.power(self.tune("shield_base", 280.) + stat.magic_power as f64 * self.tune("shield_ratio", 100.) / 100.));
        }
        if now % 30 != 0 {
            return;
        }
        if let Some(force) = self.spec.stats.get("adaptive_force") {
            let copies = rules::inventory(ctx, id)
                .iter()
                .filter(|key| rules::base(key) == self.spec.key)
                .count()
                .max(1) as i32;
            let per_copy = *force;
            let force = per_copy * copies;
            self.buff(
                ctx,
                id,
                "adaptive",
                if stat.magic_power > stat.attack {
                    BuffV1 {
                        magic_power: force,
                        ..Default::default()
                    }
                } else {
                    BuffV1 {
                        attack: (per_copy as f64 * 0.6).round() as i32 * copies,
                        ..Default::default()
                    }
                },
                1.,
            );
        }
        match self.spec.key.as_str() {
            "guardian_helmet"
                if now.saturating_sub(self.last_damage) > ticks(self.tune("out_of_combat", 5.))
                    && now.saturating_sub(self.last_attack) > ticks(self.tune("out_of_combat", 5.)) =>
            {
                ctx.heal(id, id, (max as f64 * self.tune("heal_per_second", 2.5) / 200.) as usize)
            }
            "holy_crystal" => {
                let low = self.tune("level_one", 21.) as i32;
                let high = self.tune("level_max", 35.) as i32;
                let level = level.min(15).saturating_sub(1) as i32;
                self.buff(
                    ctx,
                    id,
                    "mystery",
                    BuffV1 {
                        magic_power_mult: low + (high - low) * level / 14,
                        ..Default::default()
                    },
                    1.,
                );
            }
            "rose_gold_meteor" => {
                let n = (stat.attack / 4).min(self.tune("resist_cap", 35.) as usize) as i32;
                self.buff(
                    ctx,
                    id,
                    "scale",
                    BuffV1 {
                        defence: n,
                        magic_resistance: n,
                        ..Default::default()
                    },
                    1.,
                );
            }
            "queens_wings" => self.buff(
                ctx,
                id,
                "defiance",
                BuffV1 {
                    attack_mult: ((max.saturating_sub(hp)) * 25 / max.max(1)).min(self.tune("stat_cap", 15.) as usize) as i32,
                    magic_power_mult: ((max.saturating_sub(hp)) * 25 / max.max(1)).min(self.tune("stat_cap", 15.) as usize) as i32,
                    ..Default::default()
                },
                1.,
            ),
            "thunder_belt" if preset == Preset::Default => self.buff(
                ctx,
                id,
                "permanent",
                BuffV1 {
                    defence: self.permanent as i32,
                    magic_resistance: self.permanent as i32,
                    ..Default::default()
                },
                1.,
            ),
            "rapid_boots" => self.buff(
                ctx,
                id,
                "rapid",
                BuffV1 {
                    move_speed_mult: if now.saturating_sub(self.last_attack) > ticks(self.tune("out_of_combat", 5.))
                        && now.saturating_sub(self.last_damage) > ticks(self.tune("out_of_combat", 5.))
                    {
                        self.tune("movement_speed", 10.) as i32
                    } else {
                        0
                    },
                    ..Default::default()
                },
                1.,
            ),
            "warrior_boots" => self.buff(
                ctx,
                id,
                "valor",
                BuffV1 {
                    defence: self.stacks.min(self.tune("stack_cap", 5.) as usize) as i32 * self.tune("armor_per_stack", 3.) as i32,
                    ..Default::default()
                },
                1.,
            ),
            _ => {}
        }
        if matches!(
            self.spec.key.as_str(),
            "cursed_helmet" | "molten_essence" | "dominance_ice" | "black_ice_shield"
        ) {
            let family = if matches!(self.spec.key.as_str(), "cursed_helmet" | "molten_essence") {
                "burning_soul"
            } else {
                "fortress"
            };
            if !rules::active(ctx, id, &self.key(), family) {
                return;
            }
            let targets = Self::targets(
                ctx,
                id,
                id,
                if self.spec.key == "black_ice_shield" {
                    (self.tune("radius", 42.6) * 1000.).round() as u64
                } else {
                    60000
                },
            );
            if matches!(self.spec.key.as_str(), "dominance_ice" | "black_ice_shield") {
                let n = (targets
                    .iter()
                    .filter(|&&t| ctx.get_entity(t).is_some_and(|e| e.is_champion()))
                    .count()
                    * if self.spec.key == "black_ice_shield" {
                        self.tune("resist_per_enemy", 3.) as usize
                    } else {
                        self.tune("resist_per_enemy", 6.) as usize
                    })
                .min(if self.spec.key == "black_ice_shield" {
                    self.tune("resist_cap", 15.) as usize
                } else {
                    self.tune("resist_cap", 30.) as usize
                }) as i32;
                self.buff(
                    ctx,
                    id,
                    "fortress",
                    BuffV1 {
                        defence: n,
                        magic_resistance: n,
                        ..Default::default()
                    },
                    1.,
                );
            }
            for target in targets {
                if matches!(self.spec.key.as_str(), "cursed_helmet" | "molten_essence") {
                    self.procs.push_magic(
                        ctx,
                        target,
                        self.power(
                            max as f64
                                * if self.spec.key == "molten_essence" {
                                    self.tune("burn", 0.3) / 100.
                                } else {
                                    self.tune("burn", 0.6) / 100.
                                },
                        ),
                    );
                    if self.spec.key == "cursed_helmet" {
                        self.buff(
                            ctx,
                            target,
                            "curse",
                            BuffV1 {
                                defence: -(self.tune("resist_shred", 6.) as i32),
                                magic_resistance: -(self.tune("resist_shred", 6.) as i32),
                                ..Default::default()
                            },
                            self.tune("shred_time", 3.),
                        );
                    }
                } else if self.spec.key == "dominance_ice" {
                    self.buff(
                        ctx,
                        target,
                        "lifebane",
                        BuffV1 {
                            heal_reduce: 40,
                            ..Default::default()
                        },
                        1.,
                    );
                }
            }
        }
        let mut hits = Vec::new();
        self.burns.retain(|&target, (end, next)| {
            if now > *end {
                return false;
            }
            if now >= *next {
                *next = now + ticks(1.);
                hits.push(target);
            }
            true
        });
        for target in hits {
            if let Some((_, max)) = ctx.get_entity(target).map(|e| e.hp()) {
                self.procs
                    .push_magic(ctx, target, self.power(max as f64 * self.tune("burn_hp", 1.) / 100.));
            }
        }
    }
    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        if !self.owns_effects(ctx, entity) {
            return;
        }
        if !ctx.get_entity(victim).is_some_and(|e| e.is_champion()) {
            return;
        }
        match self.spec.key.as_str() {
            "sky_piercer" => {
                self.permanent = (self.permanent + self.tune("kill_stacks", 10.) as usize).min(self.tune("stack_cap", 80.) as usize);
                rules::keep_item_memory(ctx, entity, &self.spec.key, self.permanent, 0);
            }
            "bloodthirsty_king" => self.bloodthirsty_recovery(ctx, entity),
            "fleeting_time" => Self::haste_reset(ctx, entity, self.tune("ult_reduction", 30.) as usize),
            "demon_shoes" => {
                for slot in [SkillSlotV1::Skill, SkillSlotV1::Skill2] {
                    if let Some(cd) = ctx.get_entity(entity).and_then(|e| e.cooldown(slot)) {
                        ctx.entity_reduce_cooldown(entity, slot, cd.remaining * self.tune("skill_reduction", 20.) as usize / 100);
                    }
                }
            }
            _ => {}
        }
    }
    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        if !self.owns_effects(ctx, entity) {
            return;
        }
        match self.spec.key.as_str() {
            "bloodthirsty_king" => self.bloodthirsty_recovery(ctx, entity),
            "fleeting_time" => Self::haste_reset(ctx, entity, self.tune("ult_reduction", 30.) as usize),
            _ => {}
        }
    }
    fn on_dead(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if let Some(id) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|e| e.id())
        {
            if !self.owns_effects(ctx, id) {
                return;
            }
            rules::clear_transient(ctx, id);
            if self.spec.key == "sky_piercer" {
                self.permanent = self.permanent * (100 - self.tune("death_loss", 30.) as usize) / 100;
                rules::keep_item_memory(ctx, id, &self.spec.key, self.permanent, 0);
            }
            self.publish_count(ctx, player, false, true);
        }
    }
    fn on_upgrade(&mut self, _next: &str) -> u64 {
        (self.permanent.min(u32::MAX as usize) as u64) << 32
            | self.ready.min(u32::MAX as usize) as u64
    }
    fn on_upgraded_from(&mut self, _previous: &str, carry: u64) {
        self.permanent = (carry >> 32) as usize;
        self.ready = (carry & 0xffff_ffff) as usize;
    }
}
fn is_monster_target(ctx: &StableSim<'_>, target: usize) -> bool {
    ctx.get_entity(target)
        .is_some_and(|e| !e.is_champion() && !e.is_minion() && !e.is_tower())
}
include!("mlbb_registration.rs");

/// Side systems from upstream, with no League-specific burns or World Atlas payments.
pub struct MatchHooks;
impl StableMatchHook for MatchHooks {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        rules::on_match_start(sim);
        blood::reset(sim);
        crate::heal_shield_power::on_match_start(sim);
        crate::role_gear::start(sim);
        crate::item_stats::sim::EndOfMatchItems.on_match_start(sim);
    }
    fn on_match_tick(&self, sim: &mut StableSim<'_>, seed: u64) {
        rules::on_match_tick(sim);
        blood::cleanup(sim);
        crate::heal_shield_power::on_match_tick(sim);
        rules::recover_after_landing(sim);
        rules::recover_sustain(sim);
        rules::note_inventory(sim);
        crate::role_gear::tick(sim);
        crate::match_builds::on_match_tick(sim);
        crate::item_spend::on_match_tick(sim);
        crate::item_counts::on_match_tick(sim);
        crate::patches::on_match_tick(sim);
        crate::item_stats::sim::EndOfMatchItems.on_match_tick(sim, seed);
    }
}
pub(crate) fn is_final_key(key: &str) -> bool {
    let mythical = key.strip_prefix("mythical_");
    specs().iter().any(|s| {
        s.key == mythical.unwrap_or(key)
            && !s.legacy
            && (s.tier == 3 && s.next.is_empty() && mythical.is_none()
                || s.mythical && mythical.is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn every_selectable_finished_item_has_a_build_dropdown_category() {
        for spec in specs().iter().filter(|item| item.tier == 3 && !item.legacy) {
            assert!(crate::item_catalog::category_of(&spec.key).is_some(), "{}", spec.key);
        }
    }
    #[test]
    fn rc1_registry_order_is_preserved_and_forged_tier_is_populated() {
        let frozen: Vec<String> =
            serde_json::from_str(include_str!("../catalog/rc1_registry_order.json")).unwrap();
        let keys: Vec<_> = specs()
            .iter()
            .flat_map(|s| {
                if s.mythical {
                    vec![s.key.clone(), format!("mythical_{}", s.key)]
                } else {
                    vec![s.key.clone()]
                }
            })
            .collect();
        assert_eq!(&keys[..frozen.len()], frozen.as_slice());
        assert_eq!(specs().iter().filter(|s| s.tier == 2).count(), 6);
        assert_eq!(
            specs().iter().filter(|s| s.mythical && !s.legacy).count(),
            57
        );
        assert!(!is_final_key("mythical_twilight_armor"));
        assert!(is_final_key("mythical_legacy_windtalker"));
        assert!(is_final_key("mythical_bloodlust_axe"));
        assert!(!is_final_key("deadly_blade"));
    }
    #[test]
    fn finished_and_boot_tiers_are_explicit() {
        assert_eq!(make("winter_crown", false, None).tier(), 3);
        assert_eq!(make("swift_boots", false, None).tier(), 3);
        assert_eq!(make("boots", false, None).tier(), 0);
        assert_eq!(make("legion_sword", false, None).tier(), 1);
    }
    #[test]
    fn every_upgrade_is_connected_and_stronger() {
        let mut count = 0;
        for s in specs().iter().filter(|s| s.mythical) {
            count += 1;
            let base = make(&s.key, false, None);
            let myth = make(&s.key, true, None);
            assert_eq!(base.next_tier(), vec![myth.key()]);
            assert_eq!(myth.previous_tier(), vec![base.key()]);
            assert_eq!(myth.tier(), 4);
            assert!(myth.next_tier().is_empty());
            assert!(myth.price() > base.price());
            assert!(myth.stat().hp >= base.stat().hp);
            assert_eq!(
                myth.stat().skill_cooldown_mult,
                base.stat().skill_cooldown_mult + 5
            );
            assert!(myth.power(100.) > base.power(100.));
        }
        assert_eq!(count, 58);
    }
    #[test]
    fn recipe_routes_are_unique_and_resolve() {
        let mut keys = HashSet::new();
        for s in specs() {
            assert!(keys.insert(s.key.clone()));
            if s.mythical {
                assert!(keys.insert(format!("mythical_{}", s.key)));
            }
        }
        for s in specs() {
            assert!(s.price > 0);
            assert_eq!(
                s.previous.len(),
                s.previous.iter().collect::<HashSet<_>>().len()
            );
            for prev in &s.previous {
                assert!(keys.contains(prev));
                let p = specs().iter().find(|p| p.key == *prev).unwrap();
                assert!(p.next.contains(&s.key));
                assert!(p.tier < s.tier);
            }
            for next in &s.next {
                assert!(keys.contains(next));
            }
        }
        assert_eq!(keys.len(), 169);
    }
    #[test]
    fn radiant_armor_is_not_an_upgrade_prefix() {
        assert_eq!(
            crate::build_config::base_slug("radiant_armor"),
            "radiant_armor"
        );
        assert_eq!(
            crate::build_config::base_slug("mythical_radiant_armor"),
            "radiant_armor"
        );
        assert!(!is_final_key("radiant_armor"));
        assert!(is_final_key("mythical_radiant_armor"));
        assert!(!specs().iter().any(|s| s.key == "dagger"));
        assert!(specs().iter().any(|s| s.key == "mlbb_dagger"));
    }
    #[test]
    fn upgrade_carries_permanent_stacks_and_cooldown() {
        let mut base = make("sky_piercer", false, None);
        base.permanent = 70;
        base.ready = 12345;
        let carry = base.on_upgrade("mythical_sky_piercer");
        let mut myth = make("sky_piercer", true, None);
        myth.on_upgraded_from("sky_piercer", carry);
        assert_eq!(myth.permanent, 70);
        assert_eq!(myth.ready, 12345);
    }
    #[test]
    fn configuration_reaches_actual_stats_and_cost() {
        let config: crate::config::ItemConfig =
            serde_json::from_str(r#"{"price":1234,"attack":17,"hp":400}"#).unwrap();
        let item = make("blade_of_despair", true, Some(&config));
        assert_eq!(item.price(), 1234);
        assert_eq!(item.stat().attack, 17);
        assert_eq!(item.stat().hp, 400);
    }
    fn catalog() -> Vec<MlbbItem> {
        specs()
            .iter()
            .flat_map(|s| {
                let mut a = vec![make(&s.key, false, None)];
                if s.mythical {
                    a.push(make(&s.key, true, None));
                }
                a
            })
            .collect()
    }
    #[test]
    fn smart_builds_enforces_one_antiheal_item() {
        let a = make("sea_halberd", true, None);
        let b = make("glowing_wand", true, None);
        crate::smart_builds::note_mod_item("mythical_sea_halberd", &a);
        crate::smart_builds::note_mod_item("mythical_glowing_wand", &b);
        let mut budget = crate::smart_builds::Budget::empty(crate::smart_builds::fit(
            "unknown",
            crate::build_config::Role::Any,
        ));
        budget.take("mythical_sea_halberd");
        assert!(budget.rejects("mythical_glowing_wand").is_some());
    }
    #[test]
    fn smart_builds_replaces_non_catalog_picks_and_adds_one_pair_of_boots() {
        let items = catalog();
        for item in &items {
            crate::smart_builds::note_mod_item(&item.key(), item);
        }
        let mut keys = vec!["vanilla_placeholder".to_string()];
        keys.extend(items.iter().map(|i| i.key()));
        let mut build = vec![0, 0, 0, 0, 0, 0];
        let boots = keys.iter().position(|k| k == "swift_boots").unwrap();
        crate::smart_builds::enforce(
            keys.len(),
            &mut build,
            &[false; 6],
            &[],
            false,
            crate::smart_builds::fit("unknown", crate::build_config::Role::Any),
            Some(boots),
            |_| 0.,
            |i| Some(keys[i].clone()),
            |i| crate::item_catalog::category_of(crate::build_config::base_slug(&keys[i])),
            |i| is_final_key(&keys[i]),
        );
        assert!(build.iter().all(|&i| is_final_key(&keys[i])));
        assert_eq!(build.iter().collect::<HashSet<_>>().len(), 6);
        assert_eq!(
            build
                .iter()
                .filter(|&&i| crate::smart_builds::is_boots(&keys[i]))
                .count(),
            1
        );
    }
}
#[cfg(test)]
#[path = "mlbb_sim_tests.rs"]
mod sim_tests;

#[cfg(test)]
mod stack_icon_tests {
    use super::*;
    #[test]
    fn timed_icon_counts_expire_and_clear_on_death() {
        for key in ["war_axe", "clock_of_destiny", "radiant_armor", "brute_force_breastplate", "corrosion_scythe", "swift_crossbow"] {
            for upgraded in [false,true] {
                if upgraded && !specs().iter().find(|s|s.key==key).unwrap().mythical {continue;}
                let mut item=make(key,upgraded,None);
                item.combo.gain(60,3,6,120,0);
                assert_eq!(item.icon_count(179,true),Some(3),"{key}");
                assert_eq!(item.icon_count(180,true),Some(0),"{key}");
                assert_eq!(item.icon_count(70,false),Some(0),"{key}");
            }
        }
    }
    #[test]
    fn permanent_count_survives_mythical_upgrade_and_remains_visible_when_dead() {
        for key in ["thunder_belt","sky_piercer"] {
            let mut item=make(key,false,None);item.permanent=37;
            let carry=item.on_upgrade(&format!("mythical_{key}"));
            let mut upgraded=make(key,true,None);upgraded.on_upgraded_from(key,carry);
            assert_eq!(upgraded.icon_count(900,true),Some(37));
            assert_eq!(upgraded.icon_count(900,false),Some(37));
        }
    }
    #[test]
    fn warrior_counts_obey_five_stack_cap_and_idle_timeout() {
        let mut item=make("warrior_boots",false,None);item.stacks=9;item.last_damage=100;
        assert_eq!(item.icon_count(100+ticks(5.),true),Some(5));
        assert_eq!(item.icon_count(101+ticks(5.),true),Some(0));
        assert_eq!(item.icon_count(101,false),Some(0));
        assert_eq!(make("blade_armor",false,None).icon_count(100,true),None);
    }
}

#[cfg(test)] mod physical_balance_tests {
 use super::*;
 #[test] fn finished_crit_build_reaches_reviewed_attack_budget_and_config_override_still_wins(){
  let keys=["berserkers_fury","haas_claws","great_dragon_spear","blade_of_despair","malefic_roar"];
  assert_eq!(keys.iter().map(|k|make(k,false,None).stat().attack).sum::<i32>(),175);
  assert_eq!(keys.iter().map(|k|make(k,true,None).stat().attack).sum::<i32>(),235);
  let config:crate::config::ItemConfig=serde_json::from_str(r#"{"attack":99}"#).unwrap();
  assert_eq!(make("blade_of_despair",false,Some(&config)).stat().attack,99);
 }
}

pub(crate) fn statistics_final_keys()->std::collections::BTreeSet<String>{
 specs().iter().filter(|s|!s.legacy&&s.tier==3).flat_map(|s|std::iter::once(s.key.clone()).chain(s.mythical.then(||format!("mythical_{}",s.key)))).collect()
}
