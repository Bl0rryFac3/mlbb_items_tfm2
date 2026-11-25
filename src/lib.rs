use mod_api_stable::*;
use std::cell::Cell;

mod build_config;
mod build_library;
mod champion_traits;
mod hooks;
mod item_catalog;
mod item_stats;
mod item_tiers;

mod mlbb;
mod passive_presets;
mod passive_tuning;
mod multiplayer;
mod patches;
mod route_choice;
mod role_gear;
mod first_slot_rules;
mod smart_builds;
mod tactics;
mod ui;
mod utils;
mod vfx;



pub(crate) use utils::constants::*;
pub(crate) use utils::elapsed::Elapsed;
pub(crate) use utils::heal_watch::{HealWatch, SelfCastWatch};
pub(crate) use utils::immobilize_watch::ImmobilizeWatch;
pub(crate) use utils::item_meta::ItemMeta;
pub(crate) use utils::proc_queue::ProcQueue;
pub(crate) use utils::shared_riches::SharedRiches;
// Re-exported under their old names, so paths like `crate::config::ItemConfig`
// and `crate::strategy_ui::ICON_SHEET` keep working from every module.
pub(crate) use hooks::{hook, item_build_hook};
pub(crate) use ui::{item_counts, item_spend, match_builds, options_row, solo_rank_ui, strategy_ui};
pub(crate) use utils::{
    config, extra_stats, game_text, heal_shield_power, own_team_log, perf, upgrade_carry,
};
pub(crate) use vfx::sunfire;
pub(crate) use vfx::{
    add_stack, immolate_burn, immolate_wins, mark_immolate, note_immolate, refresh_buff, size_percent, sized_range, Annul,
    Energized, Eternity, Lifeline, Spellblade, SpellbladeBonus, Transmutation,
    TransmutationNumbers,
};

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn percent_of_i32(value: i32, percent: f64) -> i32 {
    (value as f64 * percent / 100.0).round() as i32
}

fn ticks(seconds: f64) -> usize {
    (seconds * TICKS_PER_SECOND).round() as usize
}

fn is_monster(entity: &StableEntity<'_, '_>) -> bool {
    !entity.is_champion() && !entity.is_tower() && !entity.is_minion()
}

const LETHALITY_BY_KEY: &[(&str, usize)] = &[
    ("axiom_arc", 18),
    ("bastionbreaker", 22),
    ("collector", 10),
    ("edge_of_night", 15),
    ("hubris", 18),
    ("opportunity", 18),
    ("mythical_axiom_arc", 18),
    ("mythical_bastionbreaker", 22),
    ("mythical_collector", 10),
    ("mythical_edge_of_night", 15),
    ("mythical_hubris", 18),
    ("mythical_opportunity", 18),
    ("mythical_serpents_fang", 15),
    ("mythical_voltaic_cyclosword", 12),
    ("serpents_fang", 15),
    ("serrated_dirk", 10),
    ("voltaic_cyclosword", 12),
];

static LETHALITY_TABLE: std::sync::OnceLock<std::collections::HashMap<String, usize>> =
    std::sync::OnceLock::new();

fn record_lethality_table(configs: &std::collections::HashMap<String, config::ItemConfig>) {
    let table = LETHALITY_BY_KEY
        .iter()
        .map(|&(key, default)| {
            let value = configs
                .get(key)
                .and_then(|cfg| cfg.effect_lethality)
                .unwrap_or(default);
            (key.to_string(), value)
        })
        .collect();
    let _ = LETHALITY_TABLE.set(table);
}

// Lethality an item balance patch has given an item in place of the
// table's (`patches::live`). Empty while no patch has.
static LETHALITY_PATCHES: std::sync::RwLock<Option<std::collections::HashMap<String, usize>>> =
    std::sync::RwLock::new(None);
static LETHALITY_PATCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Hands over the lethality of every item a patch has changed it for, in
// place of whatever was handed over before.
pub(crate) fn set_lethality_patches(patches: std::collections::HashMap<String, usize>) {
    let any = !patches.is_empty();
    if let Ok(mut held) = LETHALITY_PATCHES.write() {
        *held = any.then_some(patches);
    }
    LETHALITY_PATCHED.store(any, std::sync::atomic::Ordering::Relaxed);
}

fn total_lethality(ctx: &mut StableSim<'_>, player: usize) -> usize {
    let Some(table) = LETHALITY_TABLE.get() else {
        return 0;
    };
    let Some(keys) = ctx.get_player(player).map(|p| p.item_keys()) else {
        return 0;
    };
    if LETHALITY_PATCHED.load(std::sync::atomic::Ordering::Relaxed) {
        if let Ok(patches) = LETHALITY_PATCHES.read() {
            if let Some(patches) = patches.as_ref() {
                return keys
                    .iter()
                    .filter_map(|key| patches.get(key).or_else(|| table.get(key)))
                    .sum();
            }
        }
    }
    keys.iter().filter_map(|key| table.get(key)).sum()
}

fn has_buff(entity: &StableEntity<'_, '_>, name: &str) -> bool {
    (0..entity.buff_count()).any(|i| entity.buff_at(i).is_some_and(|b| b.name() == name))
}

fn lethality_multiplier(armor: i32, lethality: i32) -> f64 {
    if armor <= 0 || lethality <= 0 {
        return 1.0;
    }
    let effective_armor = (armor - lethality).max(0);
    (100 + armor) as f64 / (100 + effective_armor) as f64
}

#[derive(Clone, Copy)]
struct Penetration {
    tick: usize,
    caster: usize,
    target: usize,
    damage: usize,
    armor: i32,
}

thread_local! {
    static PENETRATION: Cell<Option<Penetration>> = Cell::new(None);
}

fn apply_lethality(
    ctx: &mut StableSim<'_>,
    caster: usize,
    target: usize,
    lethality: usize,
    damage: &mut usize,
) {
    let armor = ctx
        .get_entity(target)
        .map(|t| t.stat().defence as i32)
        .unwrap_or(0);
    let tick = ctx.tick();

    let already = PENETRATION.with(|cell| match cell.get() {
        Some(prev)
            if prev.tick == tick
                && prev.caster == caster
                && prev.target == target
                && prev.damage == *damage =>
        {
            prev.armor
        }
        _ => 0,
    });

    let mult = lethality_multiplier((armor - already).max(0), lethality as i32);
    *damage = (*damage as f64 * mult).round() as usize;

    PENETRATION.with(|cell| {
        cell.set(Some(Penetration {
            tick,
            caster,
            target,
            damage: *damage,
            armor: already + lethality as i32,
        }))
    });
}

fn is_enemy_champion(ctx: &mut StableSim<'_>, caster: usize, target: usize) -> bool {
    let Some(caster_team) = ctx.get_entity(caster).map(|c| c.team()) else {
        return false;
    };
    ctx.get_entity(target)
        .map(|target_ref| target_ref.is_champion() && target_ref.team() != caster_team)
        .unwrap_or(false)
}

fn apply_adaptive_force(ctx: &mut StableSim<'_>, player: usize, adaptive_force: i32, name: &str) {
    let Some((champion_id, favors_ap, already_applied)) = ctx.get_player(player).and_then(|p| {
        let champion_ref = p.champion()?;
        let stat = champion_ref.stat();
        Some((
            champion_ref.id(),
            stat.magic_power > stat.attack,
            has_buff(&champion_ref, name),
        ))
    }) else {
        return;
    };

    if already_applied {
        return;
    }

    let buff = if favors_ap {
        BuffV1 {
            magic_power: adaptive_force,
            ..BuffV1::named(name)
        }
    } else {
        BuffV1 {
            attack: (adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32,
            ..BuffV1::named(name)
        }
    };
    ctx.add_buff(champion_id, &buff);
}

// Adaptive force from a tier whose lesser tier gives some too: while the lesser
// tier's buff is still on the champion, only the difference.
fn apply_tiered_adaptive_force(
    ctx: &mut StableSim<'_>,
    player: usize,
    adaptive_force: i32,
    name: &str,
    upgrades_from: Option<(&str, i32)>,
) {
    let mut force = adaptive_force;
    if let Some((prior_buff, prior_force)) = upgrades_from {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        if has_buff(&champion_ref, prior_buff) {
            force = adaptive_force - prior_force;
        }
    }
    apply_adaptive_force(ctx, player, force, name);
}

// Enemy units (not turrets) within `range` of a spot, each as `with` makes
// it: its id, or its id and what the caller needs of it.
fn enemies_near<T>(
    ctx: &StableSim<'_>,
    team: usize,
    (x, y): (u64, u64),
    range: usize,
    with: impl Fn(&StableEntity<'_, '_>) -> T,
) -> Vec<T> {
    let range = (range * DISTANCE_UNITS_PER_RANGE) as i128;
    let range_sq = range * range;
    (0..ctx.entity_count())
        .filter_map(|index| ctx.entity_at(index))
        .filter(|e| e.is_alive() && !e.is_tower() && e.team() != team)
        .filter(|e| {
            let (ex, ey) = e.pos();
            let dx = ex as i128 - x as i128;
            let dy = ey as i128 - y as i128;
            dx * dx + dy * dy <= range_sq
        })
        .map(|e| with(&e))
        .collect()
}

// The enemies a Cleave splashes (Tiamat, Ravenous Hydra): every one other
// than `target` within `range` of it, by the game's own distance.
fn cleave_targets(ctx: &StableSim<'_>, caster_team: usize, target: usize, range: usize) -> Vec<usize> {
    let range = (range * DISTANCE_UNITS_PER_RANGE) as u64;
    let range_sq = range * range;

    let mut splashed = Vec::new();
    for index in 0..ctx.entity_count() {
        let Some(entity_ref) = ctx.entity_at(index) else {
            continue;
        };
        let id = entity_ref.id();
        if id == target {
            continue;
        }
        // Towers are enemy entities too, and Cleave is not meant for them.
        if !entity_ref.is_alive() || entity_ref.is_tower() || entity_ref.team() == caster_team {
            continue;
        }
        if ctx.distance_sq(target, id) > range_sq {
            continue;
        }
        splashed.push(id);
    }
    splashed
}

struct NativeTapExtension;

impl StableServerExtension for NativeTapExtension {
    fn before_management_tick(&self, ctx: &mut StableServerCtx<'_>) {
        tactics::driver::before_management_tick();
        // The article of an item balance patch that has just landed, where
        // the client's command for it did not get here first.
        patches::server_tick(ctx);
        // One atomic load once the server's item settings have been seen to.
        item_stats::sync_server_items(ctx);
        // And one once its game settings have the mod's match rules.
        utils::match_rules::sync_server(ctx);
        // The patch of the sets the Item Stats tab cannot place by seed: every
        // league's but the player's own.
        item_stats::place_captures(ctx);
    }

    fn after_management_tick(&self, _ctx: &mut StableServerCtx<'_>) {
        // Nothing here: the captures stay in memory and the totals live in the
        // save, which only a `StableClient` reaches (`item_stats::sync`).
    }

    fn handle_command(
        &self,
        ctx: &mut StableServerCtx<'_>,
        command: &StableCommand<'_>,
    ) -> CommandResultV1 {
        // The client's word that an item balance patch has landed: its
        // state and its article, now and not at the next management tick.
        // Or a player's builds, for every player in a multiplayer session.
        if patches::handle_command(ctx, command) || multiplayer::handle_command(ctx, command) {
            CommandResultV1::Handled
        } else {
            CommandResultV1::Pass
        }
    }

    fn on_server_start(&self, ctx: &mut StableServerCtx<'_>) {
        tactics::driver::on_server_start();
        // The players whose builds the last save's server held are not this
        // one's.
        multiplayer::server_started();
        // The mod's numbers for the game's own thirty items, into this
        // server's item settings, before it creates a match.
        item_stats::server_started();
        item_stats::sync_server_items(ctx);
        // The mod's match rules, starting gold first of them, into this
        // server's game settings, before it creates a match.
        utils::match_rules::server_started();
        utils::match_rules::sync_server(ctx);
        // What the server's side of the save has of the item patches, for
        // the test log.
        patches::server_started(ctx);
        // And its own Item Rules setting, not the last save's.
        item_tiers::server_started();

        match hook::install_hook() {
            Ok(address) => {
                eprintln!("mlbb_items: hook_installed address=0x{address:x}");
            }
            Err(error) if error == "hook already installed" => {}
            Err(error) => {
                eprintln!("mlbb_items: hook_refused error={error}");
                hook::build_status("route-hook", format!("refused: {error}"));
                // Resolution failed diagnostics
                match hook::candidate_report() {
                    Ok(candidates) => {
                        eprintln!(
                            "mlbb_items: hook_candidates count={}",
                            candidates.len()
                        );
                        for candidate in candidates {
                            eprintln!("mlbb_items: hook_candidate {candidate}");
                        }
                    }
                    Err(error) => {
                        eprintln!("mlbb_items: hook_candidates_failed error={error}");
                    }
                }
            }
        }
    }
}

fn init(host: &StableHost) -> StableMod {
    let mut reg = StableMod::new("mlbb_items");
    // Before any build path can ask: modded champions' AD/AP tags, from their
    // own files. See `champion_traits::MOD_CHAMPIONS`.
    champion_traits::load_mod_champions();
    // And the game's own thirty items, from the mod's settings file: the
    // Smart Builds rules know what each gives from here on.
    item_stats::prime_game_items();
    let (configs, skipped) = config::load();
    // What of the player's config.json no item can use, which is otherwise
    // only seen as numbers that never change.
    if !skipped.is_empty() {
        const NAMED: usize = 20;
        let more = skipped.len().saturating_sub(NAMED);
        host.log(
            LogLevel::Warn,
            &format!(
                "mlbb_items: config.json: {} value(s) not used, defaults kept: {}{}",
                skipped.len(),
                skipped[..skipped.len().min(NAMED)].join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ),
        );
    }
    record_lethality_table(&configs);

    role_gear::init();
    tactics::driver::on_mod_init();

    // What each World Atlas item pays its holder (Shared Riches), noted as the
    // items register below and handed to the match hook, which does the paying.


    mlbb::register(&mut reg, &configs);

    // `item-builds.json` hook
    reg.add_item_build_hook(item_build_hook::ConfiguredBuilds);

    reg.set_match_hook(mlbb::MatchHooks);
    reg.set_server_extension(NativeTapExtension);

    // in-game build picker
    reg.set_extension(strategy_ui::StrategyPicker);

    host.log(
        LogLevel::Info,
        &format!(
            "mlbb_items: registered items, config entries={}",
            configs.len()
        ),
    );

    reg
}

declare_stable_mod!(init);
