//! Stable-API callback tests with a deliberately small fake host, not the game engine.
use super::*;
use std::ffi::c_void;
use std::mem::size_of;
#[derive(Default)]
#[repr(C)]
struct State {
    game_id: usize,
    tick: usize,
    hp: [usize; 4],
    cd: [usize; 4],
    targetable: bool,
    buffs: Vec<BuffV1>,
    banishes: Vec<usize>,
    banish_visuals: Vec<(String, String)>,
    typed: Vec<(usize, usize, u32)>,
    inventory: Vec<String>,
    heals: Vec<(usize, usize)>,
    shields: Vec<(usize, usize)>,
    reject_banish: bool,
    reject_hp: bool,
    reject_shield: bool,
}
unsafe fn state<'a>(p: *const c_void) -> &'a mut State {
    &mut *(p as *mut State)
}
unsafe extern "C" fn tick(p: *const c_void) -> usize {
    state(p).tick
}
unsafe extern "C" fn valid(_p: *const c_void, h: EntityHandleV1) -> bool {
    h.id().is_some_and(|i| i < 4)
}
unsafe extern "C" fn player(_p: *const c_void, h: PlayerHandleV1) -> bool {
    h.id() == Some(0)
}
unsafe extern "C" fn champion(_p: *const c_void, _h: PlayerHandleV1) -> EntityHandleV1 {
    EntityHandleV1::from_id(0)
}
unsafe extern "C" fn hp(p: *const c_void, h: EntityHandleV1, a: *mut usize, b: *mut usize) -> bool {
    *a = state(p).hp[h.id().unwrap()];
    *b = 1000;
    true
}
unsafe extern "C" fn stats(_p: *const c_void, _h: EntityHandleV1, out: *mut StatV1) -> bool {
    *out = StatV1 {
        attack: 100,
        magic_power: 100,
        hp: 1000,
        ..Default::default()
    };
    true
}
unsafe extern "C" fn alive(p: *const c_void, h: EntityHandleV1) -> bool {
    state(p).hp[h.id().unwrap()] > 0
}
unsafe extern "C" fn yes(_p: *const c_void, _h: EntityHandleV1) -> bool {
    true
}
unsafe extern "C" fn targetable(p: *const c_void, _h: EntityHandleV1) -> bool {
    state(p).targetable
}
unsafe extern "C" fn level(_p: *const c_void, _h: EntityHandleV1) -> usize {
    10
}
unsafe extern "C" fn cd(
    p: *const c_void,
    _h: PlayerHandleV1,
    a: *mut usize,
    b: *mut usize,
    c: *mut usize,
    d: *mut usize,
) -> bool {
    let x = state(p).cd;
    *a = x[0];
    *b = x[1];
    *c = x[2];
    *d = x[3];
    true
}
unsafe extern "C" fn buff(p: *mut c_void, _target: usize, b: *const BuffV1) {
    state(p).buffs.push(*b)
}
unsafe extern "C" fn banish(
    p: *mut c_void,
    _caster: usize,
    _target: EntityHandleV1,
    n: usize,
    a: *const u8,
    b: usize,
    c: *const u8,
    d: usize,
) -> bool {
    if state(p).reject_banish {
        return false;
    }
    state(p).banishes.push(n);
    state(p).banish_visuals.push((
        String::from_utf8_lossy(std::slice::from_raw_parts(a, b)).into_owned(),
        String::from_utf8_lossy(std::slice::from_raw_parts(c, d)).into_owned(),
    ));
    state(p).targetable = false;
    true
}
unsafe extern "C" fn set_hp(p: *mut c_void, h: EntityHandleV1, n: usize) -> bool {
    if state(p).reject_hp {
        return false;
    }
    state(p).hp[h.id().unwrap()] = n;
    true
}
unsafe extern "C" fn typed(
    p: *mut c_void,
    _caster: usize,
    target: usize,
    n: usize,
    kind: u32,
    _attack: u32,
) -> bool {
    state(p).typed.push((target, n, kind));
    state(p).hp[target] = state(p).hp[target].saturating_sub(n);
    true
}
unsafe extern "C" fn count(_p: *const c_void) -> usize {
    1
}
unsafe extern "C" fn player_at(_p: *const c_void, n: usize) -> PlayerHandleV1 {
    PlayerHandleV1::from_id(n)
}
unsafe extern "C" fn item_count(p: *const c_void, _h: PlayerHandleV1) -> usize {
    state(p).inventory.len()
}
unsafe extern "C" fn item_key(
    p: *const c_void,
    _h: PlayerHandleV1,
    i: usize,
    out: *mut u8,
    cap: usize,
    len: *mut usize,
) -> bool {
    let Some(k) = state(p).inventory.get(i) else {
        return false;
    };
    *len = k.len();
    if out.is_null() && cap == 0 {
        return true;
    }
    if cap < k.len() {
        return false;
    }
    std::ptr::copy_nonoverlapping(k.as_ptr(), out, k.len());
    true
}
unsafe extern "C" fn heal(p: *mut c_void, _caster: usize, target: usize, amount: usize) {
    state(p).heals.push((target, amount));
    state(p).hp[target] = (state(p).hp[target] + amount).min(1000);
}
unsafe extern "C" fn shield(
    p: *mut c_void,
    target: EntityHandleV1,
    amount: usize,
    _duration: usize,
) -> bool {
    if state(p).reject_shield {
        return false;
    }
    state(p).shields.push((target.id().unwrap(), amount));
    true
}
unsafe extern "C" fn deal(
    p: *mut c_void,
    _caster: usize,
    target: usize,
    ad: usize,
    ap: usize,
    _kind: u32,
) {
    state(p)
        .typed
        .push((target, ad + ap, if ap > 0 { 1 } else { 0 }));
    state(p).hp[target] = state(p).hp[target].saturating_sub(ad + ap);
}
struct Host {
    state: Box<State>,
    table: Box<SimVtableV1>,
}
impl Host {
    fn new() -> Self {
        // The SDK vtable consists exclusively of a usize header and nullable C function pointers.
        let mut v: SimVtableV1 = unsafe { std::mem::zeroed() };
        v.size = size_of::<SimVtableV1>();
        v.tick = Some(tick);
        v.player_count = Some(count);
        v.player_at = Some(player_at);
        v.player_item_count = Some(item_count);
        v.player_item_key_at = Some(item_key);
        v.heal = Some(heal);
        v.entity_add_shield = Some(shield);
        v.deal_damage = Some(deal);
        v.entity_is_valid = Some(valid);
        v.player_is_valid = Some(player);
        v.player_champion = Some(champion);
        v.entity_hp = Some(hp);
        v.entity_stat = Some(stats);
        v.entity_is_alive = Some(alive);
        v.entity_is_champion = Some(yes);
        v.entity_is_targetable = Some(targetable);
        v.entity_level = Some(level);
        v.player_cooldowns = Some(cd);
        v.add_buff = Some(buff);
        v.entity_banish = Some(banish);
        v.entity_set_hp = Some(set_hp);
        v.deal_damage_typed = Some(typed);
        let mut host = Self {
            state: Box::new(State {
                hp: [1000; 4],
                targetable: true,
                ..Default::default()
            }),
            table: Box::new(v),
        };
        host.state.game_id = (&*host.state as *const State) as usize;
        host.run(|s| rules::on_match_start(s));
        host
    }
    fn run(&mut self, f: impl FnOnce(&mut StableSim<'_>)) {
        let mut raw = SimCtxV1 {
            size: size_of::<SimCtxV1>(),
            sim: &*self.table,
            frame: std::ptr::null(),
            state: (&mut *self.state as *mut State).cast(),
        };
        let mut sim = unsafe { StableSim::from_raw(&mut raw).unwrap() };
        f(&mut sim);
    }
}
#[test]
fn demon_hunter_applies_current_health_bonus_without_item_proc_recursion() {
    let mut host = Host::new();
    let mut item = make("demon_hunter_sword", false, None);
    let mut damage = 100;
    host.run(|s| {
        item.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        )
    });
    assert_eq!(damage, 100);
    host.run(|s| {
        item.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::Item,
            false,
        )
    });
    assert_eq!(damage, 100);
}
#[test]
fn spellblade_arms_on_skill_two_but_not_attack_cooldown() {
    let mut host = Host::new();
    let mut item = make("starlium_scythe", false, None);
    host.state.cd[0] = 30;
    host.run(|s| item.update(s, 0, 0));
    assert_eq!(item.armed, 0);
    host.state.tick = 1;
    host.state.cd[2] = 600;
    host.run(|s| item.update(s, 0, 0));
    assert!(item.armed > 1);
    let mut damage = 100;
    host.run(|s| {
        item.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        )
    });
    assert_eq!(item.armed, 0);
    host.state.tick = 2;
    host.run(|s| item.update(s, 0, 0));
    assert_eq!(host.state.typed, vec![(1, 90, DamageTypeV1::Fixed.code())]);
}
#[test]
fn wind_visual_buff_uses_immunity_duration_and_does_not_retrigger_on_cooldown() {
    let mut host = Host::new();
    host.state.hp[0] = 250;
    let mut item = make("wind_of_nature", false, None);
    host.run(|s| item.on_damaged(s, 0, 0, 1, 10, DamageTypeV1::Ad, AttackTypeV1::Skill, false));
    let wind: Vec<_> = host.state.buffs.iter()
        .filter(|b| b.name() == "mlbb_wind_of_nature_wind_chant").collect();
    assert_eq!(wind.len(), 1);
    // The unnamed fake champion is not a Range-class champion.
    assert_eq!(wind[0].duration_tick, ticks(1.));
    assert_eq!(item.ready, ticks(90.));
    host.state.tick = 1;
    host.run(|s| item.on_damaged(s, 0, 0, 1, 10, DamageTypeV1::Ad, AttackTypeV1::Skill, false));
    assert_eq!(host.state.buffs.iter().filter(|b| b.name() == "mlbb_wind_of_nature_wind_chant").count(), 1);
    assert!(host.state.banishes.is_empty());
}
#[test]
fn winter_crown_banishes_once_and_keeps_cooldown() {
    let mut host = Host::new();
    host.state.hp[0] = 250;
    let mut item = make("winter_crown", false, None);
    host.run(|s| item.update(s, 0, 0));
    assert_eq!(host.state.banishes, vec![ticks(2.)]);
    assert_eq!(host.state.banish_visuals, vec![("mlbb_winter_frozen".into(), "mlbb_winter_break".into())]);
    assert_eq!(item.ready, ticks(100.));
    host.state.tick = 200;
    host.state.targetable = true;
    host.run(|s| item.update(s, 0, 0));
    assert_eq!(host.state.banishes.len(), 1);
}
#[test]
fn immortality_waits_for_revive_delay_and_retains_rearm_timer() {
    let mut host = Host::new();
    let mut item = make("immortality", false, None);
    host.run(|s| simulate_immortal(&mut item, s));
    assert!(!host.state.buffs.iter().any(|b| b.undying));
    host.state.hp[0] = 1;
    host.state.tick = 1;
    host.run(|s| {
        rules::note_immortal_hit(crate::heal_shield_power::game_of(s), 0, 1, 1000, 0, 1000);
    });
    host.run(|s| simulate_immortal(&mut item, s));
    assert_eq!(host.state.banishes, vec![ticks(2.5)]);
    assert_eq!(host.state.banish_visuals, vec![("mlbb_immortality_hold".into(), "mlbb_immortality_return".into())]);
    host.state.tick = 100;
    host.run(|s| simulate_immortal(&mut item, s));
    assert_eq!(host.state.hp[0], 1);
    host.state.tick = 152;
    host.run(|s| simulate_immortal(&mut item, s));
    assert_eq!(host.state.hp[0], 160);
    assert_eq!(item.ready, 1 + ticks(210.));
}
#[test]
fn sky_piercer_checks_health_after_the_original_hit() {
    let mut host = Host::new();
    host.state.hp[1] = 100;
    let mut item = make("sky_piercer", false, None);
    let mut damage = 70;
    host.run(|s| {
        item.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        )
    });
    host.state.hp[1] = 30;
    host.state.tick = 1;
    host.run(|s| item.update(s, 0, 0));
    assert_eq!(host.state.hp[1], 0);
    assert_eq!(host.state.typed, vec![(1, 30, DamageTypeV1::Fixed.code())]);
}

#[test]
fn gift_recovers_over_three_seconds_and_stronger_gift_supersedes_component() {
    let mut h = Host::new();
    h.state.hp[0] = 400;
    h.state.inventory = vec!["elegant_gem".into(), "clock_of_destiny".into()];
    let mut small = make("elegant_gem", false, None);
    let mut big = make("clock_of_destiny", false, None);
    h.run(|s| {
        rules::on_match_start(s);
        small.update(s, 0, 0);
        big.update(s, 0, 0);
    });
    assert_eq!(h.state.hp[0], 400);
    assert_eq!(small.ready, 0);
    for t in 1..=180 {
        h.state.tick = t;
        h.run(|s| big.update(s, 0, 0));
    }
    assert_eq!(h.state.hp[0], 550);
    assert_eq!(h.state.heals.len(), 6);
    assert_eq!(big.ready, ticks(60.));
}
#[test]
fn oracle_and_lifebane_expire_and_are_isolated_between_matches() {
    let mut h = Host::new();
    let mut other = Host::new();
    h.run(|s| {
        rules::on_match_start(s);
        rules::note_oracle(s, 0, 25);
        rules::note_lifebane(s, 0, 120);
        rules::shield(s, 0, 100, 60);
    });
    assert_eq!(h.state.shields, vec![(0, 75)]);
    other.run(|s| {
        rules::on_match_start(s);
        rules::shield(s, 0, 100, 60);
    });
    assert_eq!(other.state.shields, vec![(0, 100)]);
    h.state.tick = 120;
    h.run(|s| {
        rules::on_match_tick(s);
        rules::shield(s, 0, 100, 60);
    });
    assert_eq!(h.state.shields[1], (0, 100));
}
#[test]
fn golden_staff_repeats_eligible_attack_effects_without_repeating_the_attack() {
    let mut h = Host::new();
    h.state.inventory = vec!["golden_staff".into(), "demon_hunter_sword".into()];
    let mut golden = make("golden_staff", false, None);
    let mut dhs = make("demon_hunter_sword", false, None);
    h.run(|s| rules::on_match_start(s));
    for t in 0..3 {
        h.state.tick = t;
        let mut damage = 100;
        h.run(|s| {
            golden.on_attack(
                s,
                0,
                1,
                &mut damage,
                DamageTypeV1::Ad,
                AttackTypeV1::BaseAttack,
                false,
            );
            dhs.on_attack(
                s,
                0,
                1,
                &mut damage,
                DamageTypeV1::Ad,
                AttackTypeV1::BaseAttack,
                false,
            );
        });
        assert_eq!(damage, 100); // extra effects are queued individually, not a 3x attack.
    }
    assert_eq!(h.state.heals, vec![(0, 50), (0, 50), (0, 150)]);
    for t in 3..15 {
        h.state.tick = t;
        h.run(|s| dhs.update(s, 0, 0));
    }
    assert_eq!(h.state.typed.len(), 5);
    assert_eq!(h.state.typed[0].1, 80);
    assert_eq!(h.state.typed[3].1, 61);
    assert_eq!(h.state.typed[4].1, 56); // second effect reads HP after the first.
}
#[test]
fn radiant_armor_accepts_item_magic_and_its_stacks_expire() {
    let mut h = Host::new();
    let mut item = make("radiant_armor", false, None);
    h.run(|s| item.on_damaged(s, 0, 0, 1, 20, DamageTypeV1::Ap, AttackTypeV1::Item, false));
    assert_eq!(item.combo.count(0), 1);
    h.state.tick = ticks(5.);
    h.run(|s| item.on_damaged(s, 0, 0, 2, 20, DamageTypeV1::Ap, AttackTypeV1::Skill, false));
    assert_eq!(item.combo.count(ticks(5.)), 1);
}

#[test]
fn native_landing_policy_blocks_physical_skills_but_preserves_magic_and_true_damage() {
    let mut h = Host::new();
    h.run(|s| {
        rules::on_match_start(s);
        rules::on_match_tick(s);
        rules::note_protection(s, 0, "physical", 120);
        let g = crate::heal_shield_power::game_of(s);
        assert_eq!(rules::landing_damage(g, 0, None, 2000, 0), 0);
        assert_eq!(rules::landing_damage(g, 0, None, 2000, 1), 2000);
        assert_eq!(rules::landing_damage(g, 0, None, 2000, 2), 2000);
    });
    h.state.tick = 120;
    h.run(|s| {
        rules::on_match_tick(s);
        assert_eq!(
            rules::landing_damage(crate::heal_shield_power::game_of(s), 0, None, 2000, 0),
            2000
        );
    });
}
#[test]
fn athena_protects_first_magic_hit_and_rearms_only_after_a_gap() {
    let mut h = Host::new();
    h.run(|s| {
        rules::on_match_start(s);
        rules::note_protection(s, 0, "athena", 2);
        assert_eq!(
            rules::landing_damage(crate::heal_shield_power::game_of(s), 0, None, 100, 1),
            75
        );
    });
    for t in 1..=660 {
        h.state.tick = t;
        h.run(|s| {
            rules::on_match_tick(s);
            rules::note_protection(s, 0, "athena", t + 2);
            if [180, 360, 660].contains(&t) {
                assert_eq!(
                    rules::landing_damage(crate::heal_shield_power::game_of(s), 0, None, 100, 1),
                    if t == 660 { 75 } else { 100 }
                );
            }
        });
    }
}
#[test]
fn flask_watches_actual_aid_and_has_a_separate_cooldown_for_each_ally() {
    let mut h = Host::new();
    let mut item = make("flask_of_the_oasis", false, None);
    h.run(|s| {
        rules::on_match_start(s);
        rules::on_match_tick(s);
        rules::note_aid(crate::heal_shield_power::game_of(s), 0, 1);
        item.update(s, 0, 0);
    });
    assert!(h.state.shields.is_empty());
    h.state.tick = 60;
    h.state.hp[1] = 300;
    h.state.hp[2] = 300;
    h.run(|s| {
        rules::on_match_tick(s);
        rules::note_aid(crate::heal_shield_power::game_of(s), 0, 2);
        item.update(s, 0, 0);
    });
    assert_eq!(h.state.shields, vec![(1, 350), (2, 350)]);
    h.state.tick = 120;
    h.run(|s| {
        rules::on_match_tick(s);
        rules::note_aid(crate::heal_shield_power::game_of(s), 0, 1);
        item.update(s, 0, 0);
    });
    assert_eq!(h.state.shields.len(), 2);
}
#[test]
fn genius_stacks_are_target_local_and_expire_without_generic_combat_resets() {
    let mut h = Host::new();
    let mut item = make("genius_wand", false, None);
    let mut damage = 100;
    for target in [1, 1, 2] {
        h.run(|s| {
            item.on_attack(
                s,
                0,
                target,
                &mut damage,
                DamageTypeV1::Ap,
                AttackTypeV1::Skill,
                false,
            )
        });
    }
    assert_eq!(item.target_stacks[&1].count(0), 2);
    assert_eq!(item.target_stacks[&2].count(0), 1);
    h.state.tick = 120;
    h.run(|s| {
        item.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ap,
            AttackTypeV1::Skill,
            false,
        )
    });
    assert_eq!(item.target_stacks[&1].count(120), 1);
}

#[test]
fn duplicate_dhs_keeps_stats_but_only_one_copy_produces_attack_effects() {
    let mut host = Host::new();
    host.state.inventory = vec!["demon_hunter_sword".into(), "demon_hunter_sword".into()];
    let mut first = make("demon_hunter_sword", false, None);
    let mut second = first.clone();
    assert_eq!(
        first.stat().attack + second.stat().attack,
        first.stat().attack * 2
    );
    host.run(|s| {
        let mut a = 100;
        first.on_attack(
            s,
            0,
            1,
            &mut a,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        );
        let mut b = 100;
        second.on_attack(
            s,
            0,
            1,
            &mut b,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        );
    });
    assert_eq!(host.state.heals.len(), 1);
    for tick in 1..=10 {
        host.state.tick = tick;
        host.run(|s| {
            first.update(s, 0, 0);
            second.update(s, 0, 0);
        });
    }
    assert_eq!(host.state.typed.len(), 1);
}
#[test]
fn mythical_effect_wins_even_when_finished_callbacks_run_first() {
    let mut host = Host::new();
    host.state.inventory = vec![
        "blade_of_despair".into(),
        "mythical_blade_of_despair".into(),
    ];
    host.state.hp[1] = 400;
    let mut finished = make("blade_of_despair", false, None);
    let mut myth = make("blade_of_despair", true, None);
    host.run(|s| {
        let mut damage = 100;
        finished.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        );
        assert_eq!(damage, 100);
        myth.on_attack(
            s,
            0,
            1,
            &mut damage,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        );
        assert_eq!(damage, 131);
    });
}
#[test]
fn unique_owner_handoff_and_rebuy_do_not_reset_winter_cooldown() {
    let mut host = Host::new();
    host.state.inventory = vec!["winter_crown".into(), "winter_crown".into()];
    host.state.hp[0] = 200;
    let mut first = make("winter_crown", false, None);
    let mut second = first.clone();
    host.run(|s| {
        first.update(s, 0, 0);
        second.update(s, 0, 0);
    });
    assert_eq!(host.state.banishes.len(), 1);
    host.state.tick = 2;
    host.state.inventory.pop();
    host.run(|s| second.update(s, 0, 0));
    assert_eq!(host.state.banishes.len(), 1);
    let mut rebought = make("winter_crown", false, None);
    host.state.tick = 4;
    host.run(|s| rebought.update(s, 0, 0));
    assert_eq!(host.state.banishes.len(), 1);
    host.state.tick = ticks(100.);
    host.state.targetable = true; // the fake host does not advance banish lifetimes
    host.run(|s| rebought.update(s, 0, 0));
    assert_eq!(host.state.banishes.len(), 2);
}

#[test]
fn manual_patcher_stat_change_reaches_carried_copies_in_the_simulation_hook() {
    let base = crate::patches::base::test_mlbb_base(serde_json::json!({}));
    let original = base.value("blade_of_despair", "attack") as i32;
    let mut patch_state = crate::patches::state::State::default();
    patch_state.stage("blade_of_despair", "attack", Some((original + 12) as f64));
    let patch = crate::patches::balance::manual(&base, &patch_state, 1, "25.1");
    patch_state.adopt(patch);
    crate::patches::live::rebuild(&base, &patch_state);
    let mut host = Host::new();
    host.state.inventory = vec![
        "blade_of_despair".into(),
        "blade_of_despair".into(),
        "mythical_blade_of_despair".into(),
    ];
    host.run(|s| crate::patches::on_match_tick(s));
    let delta = host
        .state
        .buffs
        .iter()
        .find(|buff| buff.name() == "riot_item_patch")
        .unwrap();
    assert_eq!(delta.attack, 24); // +12 for each Finished copy, no Mythical change
    let configured = crate::patches::live::config_for("blade_of_despair").unwrap();
    assert_eq!(
        make("blade_of_despair", false, Some(&configured))
            .stat()
            .attack,
        original + 12
    );
    crate::patches::live::rebuild(&base, &crate::patches::state::State::default());
}

#[test]
fn unique_permanent_stacks_survive_mythical_handoff_and_death_loss_applies_once() {
    let mut host = Host::new();
    host.state.inventory = vec!["sky_piercer".into(), "sky_piercer".into()];
    let mut first = make("sky_piercer", false, None);
    let mut second = first.clone();
    host.run(|s| {
        first.on_kill(s, 0, 0, 0, 1);
        second.on_kill(s, 0, 0, 0, 1);
    });
    assert_eq!(first.permanent, 10);
    host.run(|s| {
        first.on_dead(s, 0);
        second.on_dead(s, 0);
    });
    assert_eq!(first.permanent, 7);
    let mut mythical = make("sky_piercer", true, None);
    host.state.tick = 1;
    host.state.inventory = vec!["sky_piercer".into(), "mythical_sky_piercer".into()];
    host.run(|s| mythical.on_kill(s, 0, 0, 0, 1));
    assert_eq!(mythical.permanent, 17);
    host.state.inventory.clear();
    host.run(|s| rules::on_match_tick(s));
    let mut bought = make("sky_piercer", false, None);
    host.state.inventory = vec!["sky_piercer".into()];
    host.state.tick = 3;
    host.run(|s| bought.on_kill(s, 0, 0, 0, 1));
    assert_eq!(bought.permanent, 10);
}
#[test]
fn immortality_revive_is_not_lost_or_duplicated_when_mythical_takes_ownership() {
    let mut host = Host::new();
    host.state.inventory = vec!["immortality".into()];
    host.state.hp[0] = 1;
    let mut first = make("immortality", false, None);
    host.run(|s| simulate_immortal(&mut first, s));
    host.run(|s| {
        rules::note_immortal_hit(crate::heal_shield_power::game_of(s), 0, 0, 1, 0, 1);
        simulate_immortal(&mut first, s);
    });
    assert_eq!(host.state.banishes.len(), 1);
    host.state.tick = 1;
    host.state.inventory.push("mythical_immortality".into());
    let mut mythical = make("immortality", true, None);
    host.run(|s| {
        simulate_immortal(&mut first, s);
        simulate_immortal(&mut mythical, s);
    });
    host.state.tick = ticks(2.5);
    host.run(|s| simulate_immortal(&mut mythical, s));
    assert_eq!(host.state.hp[0], 160);
    assert_eq!(host.state.banishes.len(), 1);
    assert_eq!(host.state.shields.len(), 1);
}

// Explicit fake-native availability for algorithm tests; production uses the
// installed-hook flag and never substitutes a hardcoded success.
fn simulate_immortal(item: &mut MlbbItem, sim: &mut StableSim<'_>) {
    if item.owns_effects(sim, 0) {
        item.update_immortality(sim, 0, true);
    }
}
#[test]
fn immortality_falls_back_to_recovery_when_banish_is_rejected() {
    let mut host = Host::new();
    host.state.inventory = vec!["immortality".into()];
    host.state.hp[0] = 1;
    host.state.reject_banish = true;
    let mut item = make("immortality", false, None);
    host.run(|sim| {
        simulate_immortal(&mut item, sim);
        let game = crate::heal_shield_power::game_of(sim);
        rules::note_immortal_hit(game, 0, 0, 1, 0, 1);
        simulate_immortal(&mut item, sim);
    });
    assert_eq!(host.state.hp[0], 160);
    assert!(host.state.banishes.is_empty());
    assert_eq!(host.state.shields, vec![(0, 300)]);
}

#[test]
fn immortality_retries_only_the_rejected_part_of_recovery() {
    let mut host = Host::new();
    host.state.inventory = vec!["mythical_immortality".into()];
    host.state.hp[0] = 1;
    let mut item = make("immortality", true, None);
    host.run(|sim| {
        simulate_immortal(&mut item, sim);
        let game = crate::heal_shield_power::game_of(sim);
        assert_eq!(rules::note_immortal_hit(game, 0, 0, 1, 0, 1), 0);
    });
    host.state.tick = 2;
    host.run(|sim| simulate_immortal(&mut item, sim));
    assert_eq!(host.state.banishes, vec![ticks(2.5)]);

    host.state.tick += ticks(2.5);
    host.state.reject_hp = true;
    host.run(|sim| simulate_immortal(&mut item, sim));
    assert_eq!(host.state.hp[0], 1);
    assert_eq!(host.state.shields, vec![(0, 375)]);

    host.state.reject_hp = false;
    host.state.tick += 1;
    host.run(|sim| simulate_immortal(&mut item, sim));
    assert_eq!(host.state.hp[0], 160);
    assert_eq!(host.state.shields, vec![(0, 375)]);
}

#[test]
fn immortality_requires_a_lethal_hit_after_shields_not_merely_one_hp() {
    let mut host = Host::new();
    host.state.inventory = vec!["immortality".into()];
    host.state.hp[0] = 1;
    let mut item = make("immortality", false, None);
    host.run(|s| {
        simulate_immortal(&mut item, s);
        assert!(!rules::take_immortal_hit(s, 0));
        rules::note_immortal_hit(crate::heal_shield_power::game_of(s), 0, 0, 1, 200, 200);
        simulate_immortal(&mut item, s);
    });
    assert!(host.state.banishes.is_empty());
    host.run(|s| {rules::note_immortal_hit(crate::heal_shield_power::game_of(s), 0, 0, 1, 200, 201);});
    // An allied heal or Guardian refund between landing and the item update
    // must not turn a consumed lethal guard into a free, repeatable rescue.
    host.state.hp[0] = 350;
    host.run(|s| simulate_immortal(&mut item, s));
    assert_eq!(host.state.banishes.len(), 1);
    let mut unavailable = make("immortality", false, None);
    let mut other = Host::new();
    other.run(|s| unavailable.update_immortality(s, 0, false));
    assert!(!other.state.buffs.iter().any(|b| b.undying));
}

#[test]
fn expert_gloves_adaptive_stats_stack_even_though_unique_effects_have_one_owner() {
    let mut one = Host::new();
    one.state.inventory = vec!["expert_gloves".into()];
    let mut single = make("expert_gloves", false, None);
    one.run(|s| single.update(s, 0, 0));
    let amount = one
        .state
        .buffs
        .iter()
        .find(|b| b.name() == "mlbb_expert_gloves_adaptive")
        .unwrap()
        .attack;
    let mut two = Host::new();
    two.state.inventory = vec!["expert_gloves".into(), "expert_gloves".into()];
    let mut first = make("expert_gloves", false, None);
    let mut second = first.clone();
    two.run(|s| {
        first.update(s, 0, 0);
        second.update(s, 0, 0);
    });
    let stats = two
        .state
        .buffs
        .iter()
        .filter(|b| b.name() == "mlbb_expert_gloves_adaptive")
        .collect::<Vec<_>>();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].attack, amount * 2);
}

#[test]
fn oracle_is_primed_before_the_first_other_item_shield_callback() {
    let mut host = Host::new();
    host.state.inventory = vec!["blood_wings".into(), "oracle".into()];
    let mut blood = make("blood_wings", false, None);
    host.run(|s| {
        assert!(blood.owns_effects(s, 0));
        assert_eq!(
            rules::recipient_gain(crate::heal_shield_power::game_of(s), 0, 100, true),
            125
        );
    });
}

#[test]
fn immortality_without_lethal_events_never_refreshes_a_permanent_one_hp_guard() {
    let mut h=Host::new();h.state.inventory=vec!["immortality".into()];h.state.hp[0]=1;let mut item=make("immortality",false,None);
    for now in [0,1,60,600] {h.state.tick=now;h.run(|s|simulate_immortal(&mut item,s));}
    assert!(h.state.banishes.is_empty());assert!(!h.state.buffs.iter().any(|b|b.undying));
}
#[test]
fn native_immortality_rescues_once_then_banishes_and_stops_protecting_on_cooldown() {
    let mut h=Host::new();h.state.inventory=vec!["immortality".into()];let mut item=make("immortality",false,None);
    h.run(|s| {
        simulate_immortal(&mut item,s);
        let game=crate::heal_shield_power::game_of(s);
        assert_eq!(rules::note_immortal_hit(game,0,0,1000,200,1200),1199);
        assert_eq!(rules::note_immortal_hit(game,0,0,1,0,999),0);
    });
    h.state.hp[0]=1;h.run(|s|simulate_immortal(&mut item,s));assert_eq!(h.state.banishes,vec![ticks(2.5)]);
    h.run(|s| {assert_eq!(rules::landing_damage(crate::heal_shield_power::game_of(s),0,None,999,DamageTypeV1::Fixed.code()),0);});
    h.state.tick=ticks(2.5);h.run(|s| {rules::on_match_tick(s);simulate_immortal(&mut item,s);});
    assert_eq!(h.state.hp[0],160);assert_eq!(h.state.shields.len(),1);
    h.run(|s|assert_eq!(rules::note_immortal_hit(crate::heal_shield_power::game_of(s),0,s.tick(),160,0,200),200));
}

#[test]
fn pending_immortality_does_not_spend_winter_or_replace_the_revive_visuals() {
    let mut h=Host::new();h.state.inventory=vec!["immortality".into(),"winter_crown".into()];
    let mut immo=make("immortality",false,None);let mut winter=make("winter_crown",false,None);
    h.run(|s|{simulate_immortal(&mut immo,s);rules::note_immortal_hit(crate::heal_shield_power::game_of(s),0,s.tick(),1000,0,1000);});
    h.state.hp[0]=1;
    h.run(|s|{winter.update(s,0,0);simulate_immortal(&mut immo,s);});
    assert_eq!(winter.ready,0);assert_eq!(h.state.banishes,vec![ticks(2.5)]);
    assert_eq!(h.state.banish_visuals[0].0,"mlbb_immortality_hold");
}

#[test]
fn native_context_exists_without_flask_or_heal_power_and_tracks_the_current_sim() {
    let mut first=Host::new();let mut second=Host::new();
    first.run(|s| {
        crate::heal_shield_power::on_match_start(s);
        assert_eq!(crate::heal_shield_power::current_game(),Some(crate::heal_shield_power::game_of(s)));
        assert_eq!(crate::heal_shield_power::of(s,0),0.);
    });
    second.run(|s| {crate::heal_shield_power::on_match_tick(s);assert_eq!(crate::heal_shield_power::current_game(),Some(crate::heal_shield_power::game_of(s)));});
    let mut ordinary=make("mlbb_dagger",false,None);
    first.run(|s| {ordinary.owns_effects(s,0);assert_eq!(crate::heal_shield_power::current_game(),Some(crate::heal_shield_power::game_of(s)));assert_eq!(crate::heal_shield_power::of(s,0),0.);});
}

#[test]
fn legacy_windtalker_shares_typhoon_cooldown_and_activates_when_selected() {
    let mut host = Host::new();
    host.state.inventory = vec!["legacy_windtalker".into(), "windtalker".into()];
    let mut legacy = make("legacy_windtalker", false, None);
    let mut modern = make("windtalker", false, None);
    fn attack(item: &mut MlbbItem, ctx: &mut StableSim<'_>) {
        item.on_attack(ctx, 0, 1, &mut 100, DamageTypeV1::Ad, AttackTypeV1::BaseAttack, false);
    }
    host.run(|ctx| {
        rules::on_match_start(ctx);
        attack(&mut legacy, ctx);
        attack(&mut modern, ctx);
        assert_eq!(rules::item_ready(ctx, 0, "windtalker"), ticks(4.));
    });
    assert!(host.state.buffs.is_empty());
    host.state.inventory.pop();
    host.state.tick = 1;
    host.run(|ctx| attack(&mut legacy, ctx));
    assert!(host.state.buffs.is_empty());
    for tick in 2..=5 {
        host.state.tick = tick;
        host.run(|ctx| attack(&mut legacy, ctx));
        assert!(host.state.buffs.is_empty());
    }
    host.state.tick = ticks(4.);
    host.run(|ctx| attack(&mut legacy, ctx));
    assert!(host.state.buffs.iter().any(|buff| {
        buff.name() == "mlbb_legacy_windtalker_activate" && buff.move_speed_mult == 10
    }));
}

#[test]
fn deadly_blade_applies_shared_lifebane_on_basic_hits_only() {
    let mut host = Host::new();
    host.state.inventory = vec!["deadly_blade".into(), "sea_halberd".into()];
    let mut blade = make("deadly_blade", false, None);
    let mut halberd = make("sea_halberd", false, None);
    host.run(|ctx| {
        blade.on_attack(ctx, 0, 1, &mut 100, DamageTypeV1::Ad, AttackTypeV1::Skill, false);
    });
    assert!(host.state.buffs.is_empty());
    host.run(|ctx| {
        blade.on_attack(ctx, 0, 1, &mut 100, DamageTypeV1::Ad, AttackTypeV1::BaseAttack, false);
        halberd.on_attack(ctx, 0, 1, &mut 100, DamageTypeV1::Ad, AttackTypeV1::Skill, false);
        rules::shield(ctx, 1, 100, ticks(1.));
    });
    assert_eq!(host.state.buffs.len(), 2);
    let expected_durations = [ticks(1.5), ticks(3.)];
    for (buff, duration) in host.state.buffs.iter().zip(expected_durations) {
        assert_eq!(buff.name(), "mlbb_shared_lifebane");
        assert_eq!(buff.heal_reduce, 40);
        assert_eq!(buff.duration_tick, duration);
    }
    assert_eq!(host.state.shields, vec![(1, 60)]);
}
