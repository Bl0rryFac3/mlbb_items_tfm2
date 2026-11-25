//! Blood Wings owns its shield layer; unrelated shields never drive Guard.
use crate::tactics::heal_shield::{blood_shield, refill_blood_shield, BLOOD_SHIELD_DURATION};
use crate::{refresh_buff, ticks};
use mod_api_stable::*;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct Guard {
    amount: usize,
    initialized: bool,
    recharge_at: Option<usize>,
    burst_until: usize,
}
impl Guard {
    fn observe(&mut self, now: usize, amount: usize, recharge: usize, burst: usize) -> bool {
        if self.initialized && amount < self.amount {
            self.recharge_at = Some(now + recharge);
            if amount == 0 {
                self.burst_until = now + burst;
            }
        }
        self.amount = amount;
        !self.initialized || self.recharge_at.is_some_and(|at| now >= at)
    }
    fn granted(&mut self, amount: usize) {
        self.initialized = true;
        self.amount = amount;
        self.recharge_at = None;
    }
}
thread_local! {static GUARDS: RefCell<HashMap<(usize,u64,usize),Guard>> = RefCell::new(HashMap::new());}
const SPEED: &str = "mlbb_blood_wings_guard";
pub(super) fn reset(ctx: &StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    GUARDS.with(|g| g.borrow_mut().retain(|(id, _, _), _| *id != game));
}
pub(super) fn cleanup(ctx: &mut StableSim<'_>) {
    let game = crate::heal_shield_power::game_of(ctx);
    let gone = GUARDS.with(|guards| {
        let guards = guards.borrow();
        guards
            .keys()
            .filter(|(id, seed, entity)| {
                *id == game
                    && *seed == ctx.seed()
                    && (!ctx.get_entity(*entity).is_some_and(|e| e.is_alive())
                        || !super::rules::inventory(ctx, *entity)
                            .iter()
                            .any(|k| super::rules::base(k) == "blood_wings"))
            })
            .copied()
            .collect::<Vec<_>>()
    });
    for scope in gone {
        blood_shield(game, scope.2, true);
        ctx.entity_remove_buff(scope.2, SPEED);
        GUARDS.with(|g| {
            g.borrow_mut().remove(&scope);
        });
    }
}
pub(super) fn update(ctx: &mut StableSim<'_>, entity: usize, capacity: usize) {
    let game = crate::heal_shield_power::game_of(ctx);
    let Some(amount) = blood_shield(game, entity, false) else {
        return;
    };
    let now = ctx.tick();
    let tune = |id, default| crate::passive_tuning::value("blood_wings", id, default);
    let recharge_time = ticks(tune("rearm_time", 20.));
    let break_time = ticks(tune("break_time", 1.));
    let scope = (game, ctx.seed(), entity);
    let (recharge, burst) = GUARDS.with(|guards| {
        let mut guards = guards.borrow_mut();
        if guards.len() >= 256 && !guards.contains_key(&scope) {
            guards.clear();
        }
        let guard = guards.entry(scope).or_default();
        (guard.observe(now, amount, recharge_time, break_time), guard.burst_until)
    });
    let mut current = amount;
    if recharge {
        // Replace only Guard's remaining layer; no periodic shield stacking.
        let granted = match refill_blood_shield(game, entity, capacity) {
            Some(true) => true,
            Some(false) => super::rules::shield(ctx, entity, capacity, BLOOD_SHIELD_DURATION),
            None => false,
        };
        if granted {
            current = blood_shield(game, entity, false).unwrap_or(0);
            if current > 0 {
                GUARDS.with(|g| g.borrow_mut().entry(scope).or_default().granted(current));
            }
        }
    }
    let speed = if now < burst {
        tune("break_speed", 50.) as i32
    } else if current > 0 {
        tune("shielded_speed", 10.) as i32
    } else {
        0
    };
    if speed > 0 {
        refresh_buff(
            ctx,
            entity,
            SPEED,
            &BuffV1 {
                move_speed_mult: speed,
                ..BuffV1::timed(SPEED, 2)
            },
        );
    } else {
        ctx.entity_remove_buff(entity, SPEED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guard_waits_for_its_own_damage_and_recharges_after_the_last_hit() {
        let mut g = Guard::default();
        assert!(g.observe(0, 0, ticks(20.), ticks(1.)));
        g.granted(380);
        assert!(!g.observe(1201, 380, ticks(20.), ticks(1.))); // undamaged: no repeating refresh
        assert!(!g.observe(1300, 200, ticks(20.), ticks(1.)));
        assert_eq!(g.recharge_at, Some(2500));
        assert!(!g.observe(1400, 100, ticks(20.), ticks(1.)));
        assert_eq!(g.recharge_at, Some(2600));
        assert!(!g.observe(2599, 100, ticks(20.), ticks(1.)));
        assert!(g.observe(2600, 100, ticks(20.), ticks(1.)));
        g.granted(475);
        assert!(!g.observe(3000, 475, ticks(20.), ticks(1.)));
    }
    #[test]
    fn guard_break_speed_is_one_second_and_other_shields_do_not_enter_state() {
        let mut g = Guard::default();
        g.granted(200);
        assert!(!g.observe(10, 0, ticks(20.), ticks(1.)));
        assert_eq!(g.burst_until, 70);
        assert_eq!(g.recharge_at, Some(1210));
        assert!(!g.observe(900, 0, ticks(20.), ticks(1.)));
        assert_eq!(g.burst_until, 70);
        assert!(g.observe(1210, 0, ticks(20.), ticks(1.)));
    }
}
