use mod_api_stable::{StableDraftDecision, StableItemBuildContext, StableItemBuildHook};

use crate::item_stats::win_rates;
use crate::{build_config, smart_builds};

const MOD_ITEM_SCORE_BONUS: f32 = 0.5;

// Bonus for a support item on a support that prefers them (Smart Builds rule
// 9): above the mod's other items, below the engine's own class picks. The
// score scale is undocumented, so this is a guess.
const SUPPORT_ITEM_SCORE_BONUS: f32 = 1.0;

// Bonus for a heal, shield or buff item on a support that prefers those (Smart
// Builds rule 17).
const ALLY_AID_ITEM_SCORE_BONUS: f32 = 1.5;

pub struct ConfiguredBuilds;

impl StableItemBuildHook for ConfiguredBuilds {
    fn id(&self) -> String {
        "mlbb_items.configured_builds".to_string()
    }

    fn priority(&self) -> i32 {
        100
    }

    fn score_item(
        &self,
        ctx: &StableItemBuildContext<'_>,
        candidate: usize,
        base_score: f32,
    ) -> StableDraftDecision {
        let _probe = crate::perf::Probe::start(crate::perf::Section::ScoreItem);
        let Some(key) = ctx.item_key(candidate) else {
            return StableDraftDecision::Pass;
        };
        // Hiding a catalog card does not remove the engine's learned score.
        // Stock/retired entries must lose even when base_score is positive.
        if !crate::mlbb::active_key(key) {
            return StableDraftDecision::Replace(-1_000_000.0);
        }
        // Nudge by the item's balance patches, so how often it is built follows
        // its nerfs and buffs, and by how this champion has done with it.
        let lean = crate::patches::leaning(key) * crate::patches::LEAN_SCORE
            + win_rates::lean(lineup_table(ctx).as_deref(), ctx.champion_key(), key)
                * win_rates::LEAN_SCORE;
        // Items the engine already wants keep its ranking, plus the patch lean.
        if base_score > 0.0 {
            return if lean == 0.0 {
                StableDraftDecision::Pass
            } else {
                StableDraftDecision::Add(lean)
            };
        }
        // Boots reach a build only through Smart Builds' boots rule, never the
        // ranking.
        if !crate::strategy_ui::is_mod_final_item(key) || smart_builds::is_boots(key) {
            return StableDraftDecision::Pass;
        }
        // Only push items the champion could keep under the Smart Builds rules,
        // so mages are not pushed into Death's Dance.
        let fit = champion_fit(ctx);
        if smart_builds::Budget::empty(fit).rejects(key).is_some() {
            return StableDraftDecision::Pass;
        }
        if fit.is_preferred_aid_item(key) {
            return StableDraftDecision::Add(ALLY_AID_ITEM_SCORE_BONUS + lean);
        }
        if fit.is_preferred_support_item(key) {
            return StableDraftDecision::Add(SUPPORT_ITEM_SCORE_BONUS + lean);
        }
        StableDraftDecision::Add(MOD_ITEM_SCORE_BONUS + lean)
    }

    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let _probe = crate::perf::Probe::start(crate::perf::Section::StableBuildHook);
        let base = ctx.base_build();
        // `ctx.team()` is only a 0/1 side index, so this hook cannot tell the
        // player's side apart; `own_team_only` hands configured builds to the
        // native buy detour instead. Smart Builds still runs here, since it
        // shapes builds whoever owns them.
        let own_team_only = build_config::own_team_only_enabled();
        let configured = if own_team_only {
            None
        } else {
            self.configured_build(ctx)
        };
        let merged = configured.unwrap_or_else(|| build_config::MergedBuild {
            items: base.to_vec(),
            pinned: Vec::new(),
            reserved: Vec::new(),
        });
        let mut build = merged.items;
        // The player's athlete's build when `own_team_only` makes it differ
        // from `build`.
        let mut own = None;

        // Under `own_team_only` this build reaches both teams, so it ignores
        // pins; the pin-aware build goes to the spawn injector instead (see
        // `remember_pinned_build`).
        let later_open = !own_team_only
            && build_config::later_slot_open(&build_config::pin_row(
                ctx.champion_key(),
                champion_role(ctx),
            ));
        enforce_smart_build(ctx, &mut build, &merged.pinned, &merged.reserved, later_open, if own_team_only {crate::role_gear::Choice::shared_default()} else {build_config::gear_for(ctx.champion_key(),champion_role(ctx))});
        if own_team_only {
            own = remember_pinned_build(ctx, &build);
        }
        note_for_match_panel(ctx, &build, own.as_deref());

        if build.is_empty() || build == base {
            Vec::new()
        } else {
            build
        }
    }
}

impl ConfiguredBuilds {
    // Not team-gated: a build applies to whoever plays the champion, enemies
    // included, unless `own_team_only` is on.
    fn configured_build(
        &self,
        ctx: &StableItemBuildContext<'_>,
    ) -> Option<build_config::MergedBuild> {
        let config = build_config::load_cached();
        if config.is_empty() {
            return None;
        }

        // The host states the lane, so role builds need no guessing.
        let role = ctx
            .lane()
            .map(|lane| build_config::Role::from_lane_code(lane.code() as usize))
            .unwrap_or(build_config::Role::Any);

        build_config::build_for_champion(
            &config,
            ctx.champion_key(),
            role,
            |key| ctx.item_index(key),
            ctx.base_build(),
        )
    }
}

const SELECTABLE_FINAL_TIER: usize = 4;

fn is_selectable_final(ctx: &StableItemBuildContext<'_>, index: usize) -> bool {
    ctx.item_key(index).is_some_and(crate::mlbb::is_final_key)
}

fn champion_role(ctx: &StableItemBuildContext<'_>) -> build_config::Role {
    ctx.lane()
        .map(|lane| build_config::Role::from_lane_code(lane.code() as usize))
        .unwrap_or(build_config::Role::Any)
}

fn champion_fit(ctx: &StableItemBuildContext<'_>) -> smart_builds::Fit {
    smart_builds::fit(ctx.champion_key(), champion_role(ctx))
}

// The win-rate table this match's builds are decided with (see
// `win_rates::for_lineup`).
fn lineup_table(ctx: &StableItemBuildContext<'_>) -> Option<std::sync::Arc<win_rates::Table>> {
    win_rates::for_lineup(&ctx.ally_champions(), &ctx.enemy_champions())
}

// Under `own_team_only`, records the pin-aware build (pins merged into the
// engine's build, then the Smart Builds pass) for
// `tactics::spawn_paste_pinned_build` to give only the player's athletes.
// Returns it for the tactics panel, or `None` when no pin applies.
fn remember_pinned_build(
    ctx: &StableItemBuildContext<'_>,
    unpinned: &[usize],
) -> Option<Vec<usize>> {
    // Publishes the pin snapshot `pin_row` reads.
    build_config::load_cached();
    let mut row = build_config::pin_row(ctx.champion_key(), champion_role(ctx));
    row.resize(build_config::picker_slots(), None);
    let merged = build_config::merge_pin_row(&row, |key| ctx.item_index(key), ctx.base_build());
    if !merged.pinned.contains(&true) && merged.reserved.is_empty() && build_config::gear_for(ctx.champion_key(),champion_role(ctx))==Default::default() && !crate::role_gear::wants_roam_boots(champion_role(ctx),build_config::gear_for(ctx.champion_key(),champion_role(ctx))) {
        return None;
    }
    let later_open = build_config::later_slot_open(&row);
    let mut pinned = merged.items;
    enforce_smart_build(ctx, &mut pinned, &merged.pinned, &merged.reserved, later_open, build_config::gear_for(ctx.champion_key(),champion_role(ctx)));
    let keys = |build: &[usize]| {
        build
            .iter()
            .map(|&index| ctx.item_key(index).map(str::to_string))
            .collect::<Option<Vec<String>>>()
    };
    if let (Some(from), Some(to)) = (keys(unpinned), keys(&pinned)) {
        crate::own_team_log::line(|| {
            format!(
                "stable hook: {} lane={:?} side={:?} row={:?} shared={:?} pinned={:?}",
                ctx.champion_key(),
                champion_role(ctx),
                ctx.team(),
                row,
                from,
                to
            )
        });
        build_config::remember_pinned_build(ctx.champion_key(), row, from, to);
    }
    Some(pinned)
}

// Notes every build for the in-match tactics panel, which finds its match by
// lineup; `own` is the player's athlete's build when `own_team_only` makes it
// differ.
fn note_for_match_panel(ctx: &StableItemBuildContext<'_>, build: &[usize], own: Option<&[usize]>) {
    let keys = |build: &[usize]| {
        build
            .iter()
            .map(|&index| ctx.item_key(index).map(str::to_string))
            .collect::<Option<Vec<String>>>()
    };
    let Some(shared) = keys(build) else {
        return;
    };
    crate::match_builds::note_decision(
        ctx.champion_key(),
        ctx.lane().map(|lane| lane.code() as usize),
        &ctx.ally_champions(),
        &ctx.enemy_champions(),
        shared,
        own.and_then(keys),
    );
}

// The Smart Builds pass over a build from the host; the rules live in
// `smart_builds`, which the training-screen detour in `hook` also drives. No
// boots (rule 7) while the toggle is on "without boots".
fn enforce_smart_build(
    ctx: &StableItemBuildContext<'_>,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    later_open: bool,
    gear:crate::role_gear::Choice,
) {
    let gear=crate::first_slot_rules::apply(gear,champion_role(ctx));
    // Only this path sees the enemy lineup, which picks a tank's boots.
    let boots = (build_config::smart_boots_enabled() || crate::role_gear::wants_roam_boots(champion_role(ctx),gear)).then(|| {
        let enemies = ctx.enemy_champions();
        let boots = smart_builds::boots_for(ctx.champion_key(), champion_role(ctx), &enemies);
        build_config::remember_rule_boots(ctx.champion_key(), boots);
        boots
    });
    let table = lineup_table(ctx);
    smart_builds::enforce(
        ctx.item_count(),
        build,
        pinned,
        reserved,
        later_open,
        champion_fit(ctx),
        boots.and_then(|boots| ctx.item_index(boots)),
        |index| {
            ctx.item_key(index)
                .map_or(0.0, |key| win_rates::lean(table.as_deref(), ctx.champion_key(), key))
        },
        |index| ctx.item_key(index).map(str::to_string),
        |index| ctx.item_category(index),
        |index| is_selectable_final(ctx, index) && (gear.jungle!=crate::role_gear::Jungle::None || ctx.item_key(index).is_none_or(|k|!crate::role_gear::is_jungle_key(k))),
    );
    crate::role_gear::plan_role_gear(ctx.champion_key(),champion_role(ctx),gear,build,pinned,reserved,|k|ctx.item_index(k),|i|ctx.item_key(i).map(str::to_string));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api_stable::{ItemBuildCtxV1, StrV1};

    #[test]
    fn positive_engine_scores_cannot_keep_stock_or_retired_candidates() {
        let names = ["staff_of_rapture", "prophet_of_the_abyss", "conquerors_greatsword", "mythical_twilight_armor"];
        let keys: Vec<_> = names.iter().map(|name| StrV1::from_str(name)).collect();
        let raw = ItemBuildCtxV1 {
            size: std::mem::size_of::<ItemBuildCtxV1>(), team: 0, lane: 2,
            champion_key: StrV1::from_str("mlbb_cecilion"),
            item_keys_ptr: keys.as_ptr(), item_keys_len: keys.len(),
            item_categories_ptr: std::ptr::null(), item_tiers_ptr: std::ptr::null(),
            base_build_ptr: std::ptr::null(), base_build_len: 0,
            ally_champions_ptr: std::ptr::null(), ally_champions_len: 0,
            enemy_champions_ptr: std::ptr::null(), enemy_champions_len: 0,
        };
        let ctx = unsafe { StableItemBuildContext::from_raw(&raw).unwrap() };
        for candidate in 0..names.len() {
            assert!(matches!(ConfiguredBuilds.score_item(&ctx, candidate, 1000.0),
                StableDraftDecision::Replace(score) if score < 0.0));
        }
    }

    #[test]
    fn saved_stock_pins_are_blanks_even_when_the_catalog_resolves_them() {
        let row = vec![Some("staff_of_rapture".into()), Some("clock_of_destiny".into())];
        let keys = ["staff_of_rapture", "mythical_clock_of_destiny", "mythical_glowing_wand", "mythical_blood_wings"];
        let merged = build_config::merge_pin_row(&row,
            |key| keys.iter().position(|k| *k == key), &[2, 3]);
        assert_eq!(merged.items, [2, 1]);
        assert_eq!(merged.pinned, [false, true]);
    }

    #[test]
    fn native_route_snapshot_keeps_saved_order_and_reserved_fifth_sixth_slots() {
        use build_config::{BuildConfig, Role};
        let config: BuildConfig = serde_json::from_value(serde_json::json!({
            "mlbb_cecilion": ["clock_of_destiny", "ice_queen_wand", "glowing_wand", "lightning_truncheon", "enchanted_talisman", "blood_wings"]
        })).unwrap();
        let keys = ["staff_of_rapture", "prophet_of_the_abyss", "angels_fang", "spirit_crystal",
            "mythical_clock_of_destiny", "mythical_ice_queen_wand", "mythical_glowing_wand", "mythical_lightning_truncheon", "mythical_enchanted_talisman", "mythical_blood_wings"];
        let route = build_config::build_for_champion(&config, "mlbb_cecilion", Role::Mid,
            |key| keys.iter().position(|k| *k == key), &[0, 1, 2, 3]).unwrap();
        assert_eq!(route.items, [4, 5, 6, 7]);
        assert_eq!(route.pinned, [true; 4]);
        assert_eq!(route.reserved, [8, 9]);
        let row = config.by_champion["mlbb_cecilion"].clone();
        let from = keys[..4].iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let to = keys[4..8].iter().map(|s| s.to_string()).collect::<Vec<_>>();
        build_config::remember_pinned_build("rc8_native_snapshot_test", row.clone(), from.clone(), to.clone());
        assert_eq!(build_config::pinned_build("rc8_native_snapshot_test", &row, &from), Some(to));
        assert!(build_config::pinned_build("rc8_native_snapshot_test", &row, &["different".into()]).is_none());
    }
}
