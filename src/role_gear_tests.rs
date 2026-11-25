use super::*;
use std::{ffi::c_void, mem::size_of};
#[repr(C)]
struct State {
    game: usize,
    seed: u64,
    spawned: bool,
    tick: usize,
    gold: [usize; 3],
    inventory: [Vec<String>; 3],
    lanes: [u32; 3],
    hp: [usize; 4],
    assists: [usize; 3],
    buffs: Vec<(usize, BuffV1)>,
    heals: Vec<(usize, usize)>,
    damage: Vec<(usize, usize, u32)>,
    invisible: Vec<usize>,
}
unsafe fn st<'a>(p: *const c_void) -> &'a mut State {
    &mut *(p as *mut State)
}
unsafe extern "C" fn tick(p: *const c_void) -> usize {
    st(p).tick
}
unsafe extern "C" fn valid(_: *const c_void, h: EntityHandleV1) -> bool {
    h.id().is_some_and(|i| i < 4)
}
unsafe extern "C" fn player_valid(_: *const c_void, h: PlayerHandleV1) -> bool {
    h.id().is_some_and(|i| i < 3)
}
unsafe extern "C" fn count(_: *const c_void) -> usize {
    3
}
unsafe extern "C" fn count_entities(_: *const c_void) -> usize {
    4
}
unsafe extern "C" fn player_at(_: *const c_void, i: usize) -> PlayerHandleV1 {
    PlayerHandleV1::from_id(i)
}
unsafe extern "C" fn entity_at(_: *const c_void, i: usize) -> EntityHandleV1 {
    EntityHandleV1::from_id(i)
}
unsafe extern "C" fn champion(p: *const c_void, h: PlayerHandleV1) -> EntityHandleV1 {
    if st(p).spawned {
        EntityHandleV1::from_id(h.id().unwrap())
    } else {
        EntityHandleV1::NULL
    }
}
unsafe extern "C" fn seed(p: *const c_void) -> u64 {
    st(p).seed
}
unsafe extern "C" fn entity_name(
    _: *const c_void,
    h: EntityHandleV1,
    out: *mut u8,
    cap: usize,
    len: *mut usize,
) -> bool {
    let s = format!("hero{}", h.id().unwrap());
    *len = s.len();
    if cap == 0 {
        return true;
    }
    if cap < s.len() {
        return false;
    }
    std::ptr::copy_nonoverlapping(s.as_ptr(), out, s.len());
    true
}
unsafe extern "C" fn hp(p: *const c_void, h: EntityHandleV1, a: *mut usize, b: *mut usize) -> bool {
    *a = st(p).hp[h.id().unwrap()];
    *b = 1000;
    true
}
unsafe extern "C" fn alive(p: *const c_void, h: EntityHandleV1) -> bool {
    st(p).hp[h.id().unwrap()] > 0
}
unsafe extern "C" fn is_champ(_: *const c_void, h: EntityHandleV1) -> bool {
    h.id().unwrap() < 3
}
unsafe extern "C" fn no(_: *const c_void, _: EntityHandleV1) -> bool {
    false
}
unsafe extern "C" fn entity_team(_: *const c_void, h: EntityHandleV1) -> usize {
    if h.id().unwrap() < 2 {
        0
    } else {
        1
    }
}
unsafe extern "C" fn team(_: *const c_void, h: PlayerHandleV1) -> usize {
    if h.id().unwrap() < 2 {
        0
    } else {
        1
    }
}
unsafe extern "C" fn gold(p: *const c_void, h: PlayerHandleV1) -> usize {
    st(p).gold[h.id().unwrap()]
}
unsafe extern "C" fn add_gold(p: *mut c_void, h: PlayerHandleV1, n: i64) -> bool {
    let a = &mut st(p).gold[h.id().unwrap()];
    *a = (*a as i64 + n).max(0) as usize;
    true
}
unsafe extern "C" fn assists(p: *const c_void, h: PlayerHandleV1) -> usize {
    st(p).assists[h.id().unwrap()]
}
unsafe extern "C" fn lane(p: *const c_void, h: PlayerHandleV1) -> u32 {
    st(p).lanes[h.id().unwrap()]
}
unsafe extern "C" fn item_count(p: *const c_void, h: PlayerHandleV1) -> usize {
    st(p).inventory[h.id().unwrap()].len()
}
unsafe extern "C" fn item_key(
    p: *const c_void,
    h: PlayerHandleV1,
    i: usize,
    out: *mut u8,
    cap: usize,
    len: *mut usize,
) -> bool {
    let b = st(p).inventory[h.id().unwrap()][i].as_bytes();
    *len = b.len();
    if cap == 0 {
        return true;
    }
    if cap < b.len() {
        return false;
    }
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
    true
}
unsafe extern "C" fn near(_: *const c_void, _: usize, _: usize) -> u64 {
    0
}
unsafe extern "C" fn buff(p: *mut c_void, id: usize, b: *const BuffV1) {
    st(p).buffs.push((id, *b));
}
unsafe extern "C" fn heal(p: *mut c_void, _: usize, id: usize, n: usize) {
    st(p).heals.push((id, n));
    st(p).hp[id] = (st(p).hp[id] + n).min(1000);
}
unsafe extern "C" fn damage(
    p: *mut c_void,
    _: usize,
    id: usize,
    n: usize,
    kind: u32,
    _: u32,
) -> bool {
    st(p).damage.push((id, n, kind));
    st(p).hp[id] = st(p).hp[id].saturating_sub(n);
    true
}
unsafe extern "C" fn invisible(p: *mut c_void, h: EntityHandleV1, _: usize) -> bool {
    st(p).invisible.push(h.id().unwrap());
    true
}
struct Host {
    s: Box<State>,
    v: SimVtableV1,
}
impl Host {
    fn new() -> Self {
        let mut v: SimVtableV1 = unsafe { std::mem::zeroed() };
        v.size = size_of::<SimVtableV1>();
        v.tick = Some(tick);
        v.seed = Some(seed);
        v.entity_name = Some(entity_name);
        v.player_count = Some(count);
        v.player_at = Some(player_at);
        v.player_is_valid = Some(player_valid);
        v.player_champion = Some(champion);
        v.player_gold = Some(gold);
        v.player_add_gold = Some(add_gold);
        v.player_assists = Some(assists);
        v.player_team = Some(team);
        v.player_lane = Some(lane);
        v.player_item_count = Some(item_count);
        v.player_item_key_at = Some(item_key);
        v.entity_is_valid = Some(valid);
        v.entity_hp = Some(hp);
        v.entity_is_alive = Some(alive);
        v.entity_is_champion = Some(is_champ);
        v.entity_is_minion = Some(no);
        v.entity_is_tower = Some(no);
        v.entity_team = Some(entity_team);
        v.entity_count = Some(count_entities);
        v.entity_at = Some(entity_at);
        v.distance_sq = Some(near);
        v.add_buff = Some(buff);
        v.heal = Some(heal);
        v.deal_damage_typed = Some(damage);
        v.entity_set_invisible = Some(invisible);
        let mut h = Self {
            s: Box::new(State {
                game: 0,
                seed: 0,
                spawned: true,
                tick: 0,
                gold: [100; 3],
                inventory: std::array::from_fn(|_| vec!["arcane_boots".into()]),
                lanes: [1, 4, 2],
                hp: [1000; 4],
                assists: [0; 3],
                buffs: vec![],
                heals: vec![],
                damage: vec![],
                invisible: vec![],
            }),
            v,
        };
        h.s.game = &*h.s as *const State as usize;
        h.run(|s| {
            crate::mlbb::rules::on_match_start(s);
            start(s)
        });
        h
    }
    fn run(&mut self, f: impl FnOnce(&mut StableSim<'_>)) {
        let mut c = SimCtxV1 {
            size: size_of::<SimCtxV1>(),
            sim: &self.v,
            frame: std::ptr::null(),
            state: (&mut *self.s as *mut State).cast(),
        };
        f(&mut unsafe { StableSim::from_raw(&mut c).unwrap() });
    }
    fn choose(&mut self, id: usize, roam: Roam, earned: usize) {
        GAMES.with(|g| {
            let mut g = g.borrow_mut();
            let c = g
                .get_mut(&self.s.game)
                .unwrap()
                .carriers
                .get_mut(&id)
                .unwrap();
            c.choice.roam = roam;
            c.earned = earned;
        });
    }
}
#[test]
fn role_choice_serialization_and_numeric_validation_are_strict() {
    let cfg:crate::build_config::BuildConfig=serde_json::from_value(serde_json::json!({"mlbb_angela":{"slots":["oracle"],"gear":{"jungle":"none","roam":"favor"}}})).unwrap();
    assert_eq!(cfg.gear["mlbb_angela"].roam, Roam::Favor);
    assert!(Settings::default().valid());
    let shipped: Settings =
        serde_json::from_str(include_str!("../role-gear-default.json")).unwrap();
    assert_eq!(
        serde_json::to_value(shipped).unwrap(),
        serde_json::to_value(Settings::default()).unwrap()
    );
    let mut s = Settings::default();
    s.full_power_gold = 0;
    assert!(!s.valid());
    s = Settings::default();
    s.favor_cooldown_seconds = f64::NAN;
    assert!(!s.valid());
}
#[test]
fn hunter_only_amplifies_monsters_and_rewards_each_death_once() {
    let mut h = Host::new();
    h.s.inventory[0] = vec!["hunters_knife".into()];
    h.run(super::tick);
    assert_eq!(h.s.gold[0], 100); // The actual item never creates a second, hidden charge.
    GAMES.with(|g| {
        g.borrow_mut()
            .get_mut(&h.s.game)
            .unwrap()
            .tokens
            .insert(55, 0);
    });
    assert_eq!(landing(h.s.game, 3, Some(55), 100, 120, 0), 130);
    assert_eq!(landing(h.s.game, 2, Some(55), 100, 1000, 0), 100);
    assert_eq!(landing(h.s.game, 3, None, 100, 1000, 0), 100);
    h.s.hp[3] = 0;
    h.run(super::tick);
    assert_eq!(h.s.gold[0], 105);
    h.run(super::tick);
    assert_eq!(h.s.gold[0], 105);
}
#[test]
fn thriving_pays_only_one_roamer_per_team_and_grows_gradually() {
    let mut h = Host::new();
    h.choose(0, Roam::Encourage, 0);
    h.choose(1, Roam::Encourage, 0);
    h.s.gold = [40, 80, 100];
    h.s.tick = crate::ticks(5.);
    h.run(super::tick);
    assert_eq!(h.s.gold, [43, 80, 100]);
    let buffs: Vec<_> = h.s.buffs.iter().filter(|(_, b)| b.name() == "mlbb_roam_encourage").collect();
    assert_eq!(buffs.len(), 2);
    assert!(buffs.iter().all(|(_, b)| (b.attack, b.magic_power, b.attack_speed_mult) == (5, 7, 15)));
}
#[test]
fn encourage_auras_take_the_strongest_instead_of_stacking() {
    let mut h = Host::new();
    h.choose(0, Roam::Encourage, 192);
    h.choose(1, Roam::Encourage, 480);
    h.s.tick = 1;
    h.run(super::tick);
    let b: Vec<_> =
        h.s.buffs
            .iter()
            .filter(|(_, b)| b.name() == "mlbb_roam_encourage")
            .collect();
    assert_eq!(b.len(), 2);
    for (_, b) in b {
        assert_eq!((b.attack, b.magic_power, b.attack_speed_mult), (12, 18, 15));
    }
}
#[test]
fn favor_requires_nonself_aid_and_heals_the_lowest_hp_ally_once() {
    let mut h = Host::new();
    h.choose(0, Roam::Favor, 480);
    h.s.hp[1] = 100;
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 0, 0));
    h.run(super::tick);
    assert!(h.s.heals.is_empty());
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 0, 1));
    h.run(super::tick);
    assert_eq!(h.s.heals, [(1, 600)]);
    h.s.tick = 1;
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 0, 1));
    h.run(super::tick);
    assert_eq!(h.s.heals.len(), 1);
}
#[test]
fn dire_hit_uses_the_qualifying_hit_and_applies_split_damage_once() {
    let mut h = Host::new();
    h.choose(0, Roam::DireHit, 480);
    GAMES.with(|g| {
        g.borrow_mut()
            .get_mut(&h.s.game)
            .unwrap()
            .tokens
            .insert(55, 0);
    });
    landing(h.s.game, 2, Some(55), 100, 400, 0);
    h.s.hp[2] = 600;
    h.run(super::tick);
    assert_eq!(h.s.damage, [(2, 90, 0), (2, 90, 1)]);
    h.s.tick = 1;
    GAMES.with(|g| {
        g.borrow_mut()
            .get_mut(&h.s.game)
            .unwrap()
            .tokens
            .insert(55, 0);
    });
    landing(h.s.game, 2, Some(55), 100, 200, 0);
    h.run(super::tick);
    assert_eq!(h.s.damage.len(), 2);
}
#[test]
fn conceal_breaks_on_damage_and_recipient_cooldown_blocks_chaining() {
    let mut h = Host::new();
    h.choose(0, Roam::Conceal, 480);
    h.choose(1, Roam::Conceal, 480);
    h.run(super::tick);
    assert_eq!(h.s.invisible.len(), 2);
    GAMES.with(|g| {
        let mut g = g.borrow_mut();
        let g = g.get_mut(&h.s.game).unwrap();
        assert_eq!(g.conceal.len(), 2);
        assert_eq!(g.conceal_ready.len(), 2);
        g.revealed.insert(0);
    });
    h.s.tick = 1;
    h.s.invisible.clear();
    h.run(super::tick);
    GAMES.with(|g| assert!(!g.borrow()[&h.s.game].conceal.contains_key(&0)));
    h.s.tick = 400;
    h.run(super::tick);
    GAMES.with(|g| assert!(g.borrow()[&h.s.game].conceal.is_empty()));
}

#[test]
fn jungle_equipment_upgrades_keep_stacks_and_off_role_equipment_has_no_hunter_bonus() {
    let mut h = Host::new();
    h.s.inventory[0] = vec!["nimble_blade".into()];
    h.run(super::tick);
    for _ in 0..12 {
        h.s.hp[3] = 100;
        h.run(super::tick);
        GAMES.with(|g| {
            g.borrow_mut()
                .get_mut(&h.s.game)
                .unwrap()
                .tokens
                .insert(55, 0);
        });
        assert_eq!(landing(h.s.game, 3, Some(55), 100, 100, 0), 140);
        h.s.hp[3] = 0;
        h.run(super::tick);
    }
    GAMES.with(|g| assert_eq!(g.borrow()[&h.s.game].carriers[&0].jungle_stacks, 10));
    assert_eq!(h.s.gold[0], 196);
    h.s.inventory[0] = vec!["raptor_machete".into(), "nimble_blade".into()];
    h.run(super::tick);
    GAMES.with(|g| {
        let g = g.borrow();
        let c = &g[&h.s.game].carriers[&0];
        assert_eq!(c.jungle_key.as_deref(), Some("raptor_machete"));
        assert_eq!(c.jungle_stacks, 10);
    });
    assert!(h
        .s
        .buffs
        .iter()
        .any(|(id, b)| *id == 0 && b.name() == "mlbb_jungle_gorge" && b.attack == 14));
    h.s.lanes[0] = 2;
    h.run(super::tick);
    GAMES.with(|g| {
        let mut g = g.borrow_mut();
        let g = g.get_mut(&h.s.game).unwrap();
        assert_eq!(g.carriers[&0].jungle_stacks, 0);
        g.tokens.insert(55, 0);
    });
    assert_eq!(landing(h.s.game, 3, Some(55), 100, 1000, 0), 100);
}
#[test]
fn thriving_does_not_pay_twice_in_the_same_tick_and_requires_boots() {
    let mut h = Host::new();
    h.choose(0, Roam::Favor, 0);
    h.s.inventory[0].clear();
    h.s.tick = crate::ticks(5.);
    h.run(super::tick);
    assert_eq!(h.s.gold[0], 100);
    h.s.inventory[0] = vec!["tough_boots".into()];
    h.s.tick = crate::ticks(10.);
    h.run(super::tick);
    h.run(super::tick);
    assert_eq!(h.s.gold[0], 103);
}
#[test]
fn role_defaults_and_explicit_none_are_different() {
    use crate::build_config::Role;
    assert_eq!(
        jungle_target("unknown", Role::Jungle, Jungle::Auto),
        Some("raptor_machete")
    );
    assert_eq!(jungle_target("unknown", Role::Mid, Jungle::Auto), None);
    assert_eq!(jungle_target("unknown", Role::Jungle, Jungle::None), None);
    assert_eq!(
        resolved_roam("priest", Some(LaneV1::Support), Roam::Auto),
        Roam::Favor
    );
    assert_eq!(
        resolved_roam("unknown", Some(LaneV1::Support), Roam::Auto),
        Roam::Encourage
    );
    assert_eq!(
        resolved_roam("unknown", Some(LaneV1::Jungle), Roam::Auto),
        Roam::None
    );
    assert_eq!(
        resolved_roam("unknown", Some(LaneV1::Support), Roam::None),
        Roam::None
    );
    assert_eq!(
        resolved_roam("unknown", Some(LaneV1::Mid), Roam::Favor),
        Roam::None
    );
}
#[test]
fn priorities_use_six_slots_without_losing_pins_or_adding_seventh() {
    let mut slots = vec![
        Some("oracle".into()),
        None,
        Some("tough_boots".into()),
        Some("immortality".into()),
        None,
        Some("holy_crystal".into()),
    ];
    let c = Choice {
        jungle: Jungle::StarShard,
        jungle_first: true,
        boots_first: true,
        ..Default::default()
    };
    apply_priorities(
        &mut slots,
        c,
        Some("unknown"),
        crate::build_config::Role::Jungle,
    );
    assert_eq!(slots.len(), 6);
    assert_eq!(slots[0].as_deref(), Some("star_shard"));
    assert_eq!(slots[1].as_deref(), Some("tough_boots"));
    for k in ["oracle", "immortality", "holy_crystal"] {
        assert!(slots.iter().flatten().any(|s| s == k));
    }
    let mut full = (0..6).map(|i| Some(format!("item{i}"))).collect::<Vec<_>>();
    let original = full.clone();
    apply_priorities(
        &mut full,
        c,
        Some("unknown"),
        crate::build_config::Role::Jungle,
    );
    assert_eq!(full, original);
}
#[test]
fn automatic_jungle_plan_respects_all_pins_and_boots() {
    use crate::build_config::Role;
    let names = [
        "oracle",
        "tough_boots",
        "holy_crystal",
        "immortality",
        "blade_armor",
        "glowing_wand",
        "mythical_star_shard",
    ];
    let resolve = |k: &str| names.iter().position(|n| *n == k);
    let key = |i: usize| names.get(i).map(|s| s.to_string());
    let c = Choice {
        jungle: Jungle::StarShard,
        ..Default::default()
    };
    let mut build = vec![0, 1, 2, 3, 4, 5];
    let original = build.clone();
    plan_jungle(
        "unknown",
        Role::Jungle,
        c,
        &mut build,
        &[true; 6],
        &[],
        resolve,
        key,
    );
    assert_eq!(build, original);
    plan_jungle(
        "unknown",
        Role::Jungle,
        c,
        &mut build,
        &[true, false, false, true, true, true],
        &[],
        resolve,
        key,
    );
    assert_eq!(build, vec![0, 1, 6, 3, 4, 5]);
    plan_jungle(
        "unknown",
        Role::Jungle,
        c,
        &mut build,
        &[false; 6],
        &[],
        resolve,
        key,
    );
    assert_eq!(build.iter().filter(|i| **i == 6).count(), 1);
    assert_eq!(build.len(), 6);
}
#[test]
fn classic_branches_cannot_skip_the_knife_or_cross_into_another_branch() {
    let specs: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../catalog/equipment.json")).unwrap();
    for (part, final_key) in [
        ("curse_sword", "star_shard"),
        ("nimble_blade", "raptor_machete"),
        ("pillager_axe", "beast_killer"),
    ] {
        let p = specs.iter().find(|s| s["key"] == part).unwrap();
        let f = specs.iter().find(|s| s["key"] == final_key).unwrap();
        assert_eq!(p["previous"], serde_json::json!(["hunters_knife"]));
        assert_eq!(p["next"], serde_json::json!([final_key]));
        assert_eq!(f["previous"], serde_json::json!([part]));
        assert_eq!(f["boots"], false);
    }
}
#[test]
fn classic_combat_effects_need_real_jungle_inventory_and_do_not_echo_item_hits() {
    use mod_api_stable::StableItem;
    let mut h = Host::new();
    h.s.inventory[0] = vec!["raptor_machete".into()];
    let mut r = crate::mlbb::make("raptor_machete", false, None);
    let mut amount = 30;
    h.run(|s| {
        r.on_attack(
            s,
            0,
            2,
            &mut amount,
            DamageTypeV1::Ad,
            AttackTypeV1::BaseAttack,
            false,
        )
    });
    assert!(h.s.buffs.iter().any(|(id, b)| *id == 2 && b.defence == -7));
    let n = h.s.buffs.len();
    h.run(|s| {
        r.on_attack(
            s,
            0,
            2,
            &mut amount,
            DamageTypeV1::Ad,
            AttackTypeV1::Item,
            false,
        )
    });
    assert_eq!(h.s.buffs.len(), n);
    h.s.inventory[0] = vec!["beast_killer".into()];
    let mut b = crate::mlbb::make("beast_killer", false, None);
    h.run(|s| b.on_damaged(s, 0, 0, 2, 99, DamageTypeV1::Ad, AttackTypeV1::Skill, false));
    assert!(h.s.damage.is_empty());
    h.run(|s| b.on_damaged(s, 0, 0, 2, 1, DamageTypeV1::Ad, AttackTypeV1::Skill, false));
    assert!(!h.s.damage.is_empty());
    let n = h.s.damage.len();
    h.run(|s| b.on_damaged(s, 0, 0, 2, 200, DamageTypeV1::Ap, AttackTypeV1::Item, false));
    assert_eq!(h.s.damage.len(), n);
}

#[test]
fn late_slot_role_selection_never_duplicates_existing_or_reserved_jungle_gear() {
    use crate::build_config::Role;
    let c = Choice {
        jungle: Jungle::StarShard,
        ..Default::default()
    };
    let items = vec![
        "oracle".into(),
        "tough_boots".into(),
        "immortality".into(),
        "holy_crystal".into(),
    ];
    assert_eq!(
        extra_jungle_target("unknown", Role::Jungle, c, &items),
        Some("star_shard")
    );
    let mut reserved = items;
    reserved.push("mythical_beast_killer".into());
    assert_eq!(
        extra_jungle_target("unknown", Role::Jungle, c, &reserved),
        None
    );
    assert_eq!(
        extra_jungle_target("unknown", Role::Jungle, Choice::disabled(), &[]),
        None
    );
    assert!(crate::route_choice::is_jungle_starter("hunters_knife"));
}

#[test]
fn roaming_selection_requires_boots_with_auto_limited_to_supports() {
    use crate::build_config::Role;
    for roam in [Roam::Favor, Roam::Encourage, Roam::Conceal, Roam::DireHit] {
        assert!(!wants_roam_boots(
            Role::Mid,
            Choice {
                roam,
                ..Default::default()
            }
        ));
    }
    assert!(wants_roam_boots(Role::Support, Choice::default()));
    assert!(!wants_roam_boots(Role::Jungle, Choice::default()));
    assert!(!wants_roam_boots(Role::Support, Choice::disabled()));
}
#[test]
fn automatic_roaming_boots_preserve_six_pins_and_existing_or_reserved_pairs() {
    use crate::build_config::Role;
    let names = [
        "oracle",
        "holy_crystal",
        "immortality",
        "glowing_wand",
        "blade_armor",
        "star_shard",
        "tough_boots",
        "magic_shoes",
    ];
    let key = |i: usize| names.get(i).map(|s| s.to_string());
    let c = Choice {
        roam: Roam::Favor,
        ..Default::default()
    };
    let original = vec![0, 1, 2, 3, 4, 5];
    let mut build = original.clone();
    plan_roam_boots(Role::Support, c, &mut build, &[true; 6], &[], Some(6), key);
    assert_eq!(build, original);
    plan_roam_boots(
        Role::Support,
        c,
        &mut build,
        &[true, false, true, true, true, true],
        &[],
        Some(6),
        key,
    );
    assert_eq!(build, vec![0, 6, 2, 3, 4, 5]);
    assert_eq!(build.len(), 6);
    let mut reserved = original.clone();
    plan_roam_boots(
        Role::Support,
        c,
        &mut reserved,
        &[false; 6],
        &[7],
        Some(6),
        key,
    );
    assert_eq!(reserved, original);
    let mut held = vec![0, 7, 2, 3, 4, 5];
    let before = held.clone();
    plan_roam_boots(Role::Support, c, &mut held, &[false; 6], &[], Some(6), key);
    assert!(held.contains(&7));
    assert_eq!(held.len(), before.len());
    let mut none = original.clone();
    plan_roam_boots(
        Role::Support,
        Choice::disabled(),
        &mut none,
        &[false; 6],
        &[],
        Some(6),
        key,
    );
    assert_eq!(none, original);
}
#[test]
fn roaming_boots_first_preserves_the_jungle_slot_and_other_pinned_positions() {
    use crate::build_config::Role;
    let names = [
        "star_shard",
        "oracle",
        "holy_crystal",
        "immortality",
        "blade_armor",
        "glowing_wand",
        "magic_shoes",
    ];
    let key = |i: usize| names.get(i).map(|s| s.to_string());
    let c = Choice {
        roam: Roam::Favor,
        boots_first: true,
        jungle_first: true,
        ..Default::default()
    };
    let mut build = vec![0, 1, 2, 3, 4, 5];
    plan_roam_boots(
        Role::Support,
        c,
        &mut build,
        &[false, false, true, true, true, true],
        &[],
        Some(6),
        key,
    );
    assert_eq!(build, vec![0, 6, 2, 3, 4, 5]);
    let mut existing = vec![0, 1, 2, 3, 4, 6];
    plan_roam_boots(
        Role::Support,
        c,
        &mut existing,
        &[false, false, true, true, true, false],
        &[],
        Some(6),
        key,
    );
    assert_eq!(existing, vec![0, 6, 2, 3, 4, 1]);
}

#[test]
fn boots_first_can_lead_a_jungler_without_losing_its_unpinned_jungle_item() {
    use crate::build_config::Role;
    let names = [
        "star_shard",
        "oracle",
        "holy_crystal",
        "immortality",
        "blade_armor",
        "glowing_wand",
        "magic_shoes",
    ];
    let key = |i: usize| names.get(i).map(|s| s.to_string());
    let c = Choice {
        roam: Roam::Favor,
        boots_first: true,
        ..Default::default()
    };
    let mut build = vec![0, 1, 2, 3, 4, 5];
    plan_roam_boots(
        Role::Support,
        c,
        &mut build,
        &[false, false, true, true, true, true],
        &[],
        Some(6),
        key,
    );
    assert_eq!(build, vec![6, 0, 2, 3, 4, 5]);
    assert!(wants_roam_boots(Role::Support, Choice::shared_default()));
}

fn ui_status(roam: Roam, earned: usize) -> RoamStatus {
    RoamStatus {
        roam,
        progress_known: true,
        boots: true,
        boot_slots: vec![],
        alive: true,
        earned,
        unlock: 192,
        full_power: 480,
        ready_at: 0,
        concealed_until: 0,
    }
}
#[test]
fn roaming_display_distinguishes_missing_boots_lock_readiness_and_cooldown() {
    let mut s = ui_status(Roam::Favor, 90);
    assert_eq!(s.display(0).0, "Favor · Growth 90/480\nReady");
    s.boots = false;
    assert!(s.display(0).0.ends_with("Needs boots"));
    s.boots = true;
    s.earned = 192;
    assert_eq!(s.display(0).0, "Favor · Growth 192/480\nReady");
    s.ready_at = crate::ticks(15.);
    assert!(s.display(crate::ticks(1.)).0.ends_with("Cooldown 14s"));
    assert!(s.display(s.ready_at).0.ends_with("Ready"));
    s.alive = false;
    assert!(s.display(0).0.ends_with("Dead"));
    assert_eq!(ui_status(Roam::None, 0).display(0).0, "Roam: None\nOff");
}
#[test]
fn aura_and_conceal_display_actual_activity_and_configured_thresholds() {
    assert!(ui_status(Roam::Encourage, 192)
        .display(0)
        .0
        .ends_with("Aura active"));
    let mut s = ui_status(Roam::Conceal, 300);
    s.concealed_until = crate::ticks(5.);
    s.ready_at = crate::ticks(80.);
    assert!(s.display(0).0.ends_with("Active 5s"));
    assert!(s.display(crate::ticks(5.)).0.ends_with("Cooldown 75s"));
    s.unlock = 400;
    s.full_power = 700;
    assert_eq!(s.display(0).0, "Conceal · Growth 300/700\nActive 5s");
}
#[test]
fn roaming_history_does_not_show_future_unlocks_or_other_seats() {
    let mut t = RoamTimeline::default();
    t.record(
        42,
        5,
        0,
        vec![
            (0, 4, ui_status(Roam::Favor, 0)),
            (1, 4, ui_status(Roam::DireHit, 90)),
        ],
    );
    t.record(42, 5, 300, vec![(0, 4, ui_status(Roam::Favor, 192))]);
    assert_eq!(t.at(42, 0, 4, 60).unwrap().earned, 0);
    assert_eq!(t.at(42, 0, 4, 300).unwrap().earned, 192);
    assert_eq!(t.at(42, 1, 4, 300).unwrap().roam, Roam::DireHit);
    assert!(t.at(7, 0, 4, 300).is_none());
    assert!(t.at(42, 0, 3, 300).is_none());
    assert!(t.at(42, 2, 4, 300).is_none());
    assert!(t.at(42, 0, 4, 900).is_none());
}
#[test]
fn roaming_history_rewind_replay_and_same_tick_replace_stale_status() {
    let mut t = RoamTimeline::default();
    t.record(42, 5, 0, vec![(0, 4, ui_status(Roam::Favor, 0))]);
    t.record(42, 5, 600, vec![(0, 4, ui_status(Roam::Favor, 480))]);
    t.record(42, 5, 300, vec![(0, 4, ui_status(Roam::Favor, 90))]);
    assert_eq!(t.at(42, 0, 4, 300).unwrap().earned, 90);
    assert!(t.seats[0][4].iter().all(|(at, _)| *at <= 300));
    t.record(42, 5, 300, vec![(0, 4, ui_status(Roam::Favor, 99))]);
    assert_eq!(t.at(42, 0, 4, 300).unwrap().earned, 99);
    t.record(42, 6, 0, vec![(1, 4, ui_status(Roam::Encourage, 0))]);
    assert!(t.at(42, 0, 4, 0).is_none());
    assert_eq!(t.at(42, 1, 4, 0).unwrap().roam, Roam::Encourage);
    t.record(99, 6, 0, vec![]);
    assert!(t.at(42, 1, 4, 0).is_none());
}
#[test]
fn boot_catalogue_copy_does_not_list_unselected_roaming_effects() {
    let copy: serde_json::Value =
        serde_json::from_str(include_str!("../catalog/player_tooltips.json")).unwrap();
    for key in [
        "arcane_boots",
        "demon_shoes",
        "magic_shoes",
        "rapid_boots",
        "swift_boots",
        "tough_boots",
        "warrior_boots",
    ] {
        let text = copy[key]["base"].as_str().unwrap();
        assert!(text.contains("Support role only"));
        for label in ["Favor:", "Encourage:", "Conceal:", "Dire Hit:"] {
            assert!(!text.contains(label));
        }
    }
}
#[test]
fn roam_first_defaults_on_migrates_old_rows_and_can_be_switched_off() {
    use crate::build_config::Role;
    let old: Choice =
        serde_json::from_value(serde_json::json!({"roam":"favor","boots_first":false})).unwrap();
    assert!(old.roam_first);
    let names = ["oracle", "holy_crystal", "immortality", "magic_shoes"];
    let key = |i: usize| names.get(i).map(|s| s.to_string());
    let mut on = vec![0, 1, 2];
    plan_roam_boots(Role::Support, old, &mut on, &[false; 3], &[], Some(3), key);
    assert_eq!(on[0], 3);
    let off = Choice {
        roam_first: false,
        ..old
    };
    let mut later = vec![0, 1, 2];
    plan_roam_boots(
        Role::Support,
        off,
        &mut later,
        &[false; 3],
        &[],
        Some(3),
        key,
    );
    assert_eq!(later, vec![0, 3, 2]);
    for role in [Role::Top, Role::Jungle, Role::Mid, Role::Bottom] {
        let mut other = vec![0, 1, 2];
        plan_roam_boots(role, old, &mut other, &[false; 3], &[], Some(3), key);
        assert_eq!(other, vec![0, 1, 2]);
        assert_eq!(resolved_for_role("unknown", role, Roam::Favor), Roam::None);
    }
}

#[test]
fn missing_optional_origin_uses_only_verified_native_render_context() {
    assert!(visible_origin(None, true));
    assert!(!visible_origin(None, false));
    assert!(visible_origin(Some(SimOriginKindV1::Unknown.code()), true));
    assert!(!visible_origin(
        Some(SimOriginKindV1::ServerPresim.code()),
        true
    ));
    assert!(!visible_origin(Some(SimOriginKindV1::Tool.code()), true));
    assert!(visible_origin(
        Some(SimOriginKindV1::ClientMatchView.code()),
        false
    ));
}
#[test]
fn late_spawn_and_native_choice_bind_from_item_updates_without_optional_origin() {
    use crate::build_config::Role;
    let mut h = Host::new();
    h.s.seed = 891001;
    h.s.spawned = false;
    h.run(|s| start(s));
    GAMES.with(|g| assert!(g.borrow()[&h.s.game].carriers.is_empty()));
    seat(
        h.s.seed,
        0,
        "hero1",
        Role::Support,
        Choice {
            roam: Roam::Favor,
            ..Default::default()
        },
    );
    h.s.spawned = true;
    h.run(item_tick);
    GAMES.with(|g| {
        let g = g.borrow();
        let c = &g[&h.s.game].carriers[&1];
        assert!(c.bound);
        assert_eq!(c.choice.roam, Roam::Favor);
        assert_eq!(c.lane, Some(LaneV1::Support));
    });
    h.s.tick = crate::ticks(5.);
    h.run(item_tick);
    let gold = h.s.gold;
    h.run(item_tick);
    assert_eq!(h.s.gold, gold);
}
#[test]
fn native_choice_without_live_progress_is_specific_and_never_claims_ready() {
    use crate::build_config::Role;
    let seed = 891002;
    seat(
        seed,
        0,
        "roamer_blue",
        Role::Support,
        Choice {
            roam: Roam::Favor,
            ..Default::default()
        },
    );
    seat(
        seed,
        1,
        "roamer_red",
        Role::Support,
        Choice {
            roam: Roam::Conceal,
            ..Default::default()
        },
    );
    let blue = shown_roam_status(Some(seed), 0, 4, None).unwrap();
    let red = shown_roam_status(Some(seed), 1, 4, None).unwrap();
    assert_eq!(blue.roam, Roam::Favor);
    assert_eq!(red.roam, Roam::Conceal);
    assert!(!blue.progress_known);
    let text = selected_roam_description(&blue, 0);
    assert!(text.contains("Favor"));
    assert!(!text.contains("Conceal"));
    assert!(!text.contains("Ready"));
    assert!(shown_roam_status(Some(seed), 0, 3, None).is_none());
    assert!(shown_roam_status(Some(seed + 1), 0, 4, None).is_none());
}
#[test]
fn roam_first_reorders_full_runtime_pins_without_rewriting_saved_preset() {
    use crate::build_config::Role;
    let saved = (0..6)
        .map(|i| Some(format!("combat_{i}")))
        .collect::<Vec<_>>();
    let before = saved.clone();
    let choice = Choice {
        roam: Roam::Favor,
        ..Default::default()
    };
    let runtime = roaming_first_row("hero", Role::Support, choice, &saved);
    assert_eq!(runtime.len(), 6);
    assert!(runtime[0].as_deref().is_some_and(is_completed_boot));
    assert_eq!(runtime[1..], saved[..5]);
    assert_eq!(saved, before);
    assert_eq!(
        roaming_first_row("hero", Role::Support, choice, &runtime),
        runtime
    );
    assert_eq!(
        ordered_first_row(
            "hero",
            Role::Support,
            Choice {
                roam_first: false,
                ..choice
            },
            &saved
        ),
        saved
    );
    for role in [Role::Top, Role::Mid, Role::Bottom] {
        assert_eq!(roaming_first_row("hero", role, choice, &saved), saved);
    }
    let mut with_boot = saved.clone();
    with_boot[4] = Some("tough_boots".into());
    let reordered = roaming_first_row("hero", Role::Support, choice, &with_boot);
    assert_eq!(reordered[0].as_deref(), Some("tough_boots"));
    let mut a = with_boot.into_iter().flatten().collect::<Vec<_>>();
    let mut b = reordered.into_iter().flatten().collect::<Vec<_>>();
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn native_lane_binding_accepts_display_names_but_rejects_ambiguous_seats() {
    use crate::build_config::Role;
    let seed = 891004;
    seat(
        seed,
        0,
        "mlbb_angela",
        Role::Support,
        Choice {
            roam: Roam::Favor,
            ..Default::default()
        },
    );
    assert_eq!(
        native_seat(seed, 0, "Angela", Some(LaneV1::Support))
            .unwrap()
            .champion,
        "mlbb_angela"
    );
    assert!(native_seat(seed, 1, "Angela", Some(LaneV1::Support)).is_none());
    assert!(native_seat(seed, 0, "Angela", Some(LaneV1::Mid)).is_none());
    seat(
        seed,
        0,
        "another_support",
        Role::Support,
        Choice {
            roam: Roam::Conceal,
            ..Default::default()
        },
    );
    assert!(native_seat(seed, 0, "Angela", Some(LaneV1::Support)).is_none());
    assert_eq!(
        native_seat(seed, 0, "mlbb_angela", Some(LaneV1::Support))
            .unwrap()
            .choice
            .roam,
        Roam::Favor
    );
}

// Lifecycle regressions: no manual choice/progress injection, no optional
// sim_origin callback. Bind after match start with zero roaming gold.
fn unlocked_roamer(seed_id: u64, roam: Roam) -> Host {
    use crate::build_config::Role;
    let mut h = Host::new();
    h.s.seed = seed_id;
    h.s.inventory[1] = vec!["tough_boots".into()];
    h.run(|s| start(s));
    seat(
        seed_id,
        0,
        "hero1",
        Role::Support,
        Choice {
            roam,
            ..Default::default()
        },
    );
    h.run(item_tick);
    assert_eq!(h.s.gold[1],100);
    GAMES.with(|g| assert_eq!(g.borrow()[&h.s.game].carriers[&1].earned,0));
    h
}
#[test]
fn favor_works_at_zero_gold_after_late_binding_without_match_hooks() {
    let mut h = unlocked_roamer(891010, Roam::Favor);
    h.s.hp[0] = 100;
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 1, 1));
    h.s.tick += 1;
    h.run(item_tick);
    assert!(h.s.heals.is_empty());
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 1, 0));
    h.s.tick += 1;
    h.run(item_tick);
    assert_eq!(h.s.heals, vec![(0, 240)]);
    h.run(|s| crate::mlbb::rules::note_aid(game_id(s), 1, 0));
    h.s.tick += 1;
    h.run(item_tick);
    assert_eq!(h.s.heals.len(), 1);
}
#[test]
fn encourage_applies_aura_at_zero_gold_after_late_binding() {
    let h = unlocked_roamer(891011, Roam::Encourage);
    let auras =
        h.s.buffs
            .iter()
            .filter(|(_, b)| b.name() == "mlbb_roam_encourage")
            .collect::<Vec<_>>();
    assert_eq!(auras.len(), 2);
    for (_, b) in auras {
        assert_eq!((b.attack, b.magic_power, b.attack_speed_mult), (5, 7, 15));
    }
}
#[test]
fn dire_hit_procs_split_damage_at_zero_gold_after_late_binding() {
    let mut h = unlocked_roamer(891012, Roam::DireHit);
    GAMES.with(|g| {
        g.borrow_mut()
            .get_mut(&h.s.game)
            .unwrap()
            .tokens
            .insert(777, 1);
    });
    h.s.hp[2] = 300;
    assert_eq!(landing(h.s.game, 2, Some(777), 100, 400, 0), 100);
    h.s.tick += 1;
    h.run(item_tick);
    assert_eq!(h.s.damage, vec![(2, 35, 0), (2, 35, 1)]);
}
#[test]
fn conceal_activates_then_breaks_on_damage_at_zero_gold_after_late_binding() {
    let mut h = unlocked_roamer(891013, Roam::Conceal);
    assert!(h.s.invisible.contains(&0) && h.s.invisible.contains(&1));
    assert!(h
        .s
        .buffs
        .iter()
        .any(|(_, b)| b.name() == "mlbb_roam_conceal" && b.move_speed_mult == 30));
    landing(h.s.game, 1, None, 100, 1000, 0);
    h.s.tick += 1;
    h.run(item_tick);
    GAMES.with(|g| assert!(!g.borrow()[&h.s.game].conceal.contains_key(&1)));
}

#[test]
fn global_jungle_first_handles_six_pins_without_seventh_slot_and_off_restores_raw_order(){
 use crate::{build_config::Role,first_slot_rules::{self,Settings}};
 let raw=(0..6).map(|i|Some(format!("combat_{i}"))).collect::<Vec<_>>();
 let selected=Choice{jungle:Jungle::StarShard,roam:Roam::None,..Default::default()};
 let on=first_slot_rules::apply_with(selected,Role::Jungle,Settings{jungle_first:true,roam_first:false});
 let ordered=ordered_first_row("hero",Role::Jungle,on,&raw);
 assert_eq!(ordered.len(),6);assert_eq!(ordered[0].as_deref(),Some("star_shard"));assert_eq!(ordered[1..],raw[..5]);
 let off=first_slot_rules::apply_with(selected,Role::Jungle,Settings{jungle_first:false,roam_first:true});
 assert_eq!(ordered_first_row("hero",Role::Jungle,off,&raw),raw);
 let none=Choice{jungle:Jungle::None,..on};assert_eq!(ordered_first_row("hero",Role::Jungle,none,&raw),raw);
}

#[test]
fn role_gear_final_order_overrides_ai_boots_position_for_both_roles(){
 use crate::build_config::Role;
 let names=["tough_boots","holy_crystal","blade_armor","star_shard"];
 let resolve=|s:&str|names.iter().position(|k|*k==s);
 let key=|i:usize|names.get(i).map(|s|s.to_string());
 let mut jungle=vec![0,1,2,3];
 let choice=Choice{jungle:Jungle::StarShard,jungle_first:true,roam:Roam::None,..Default::default()};
 plan_role_gear("hero",Role::Jungle,choice,&mut jungle,&[],&[],resolve,key);
 assert_eq!(jungle[0],3);assert_eq!(jungle.len(),4);
 let mut roam=vec![1,2,0,3];
 let choice=Choice{jungle:Jungle::None,roam:Roam::Favor,roam_first:true,..Default::default()};
 plan_role_gear("hero",Role::Support,choice,&mut roam,&[],&[],resolve,key);
 assert_eq!(roam[0],0);assert_eq!(roam.len(),4);
}
