//! Native MLBB recipient, typed landing and penetration policies for the exact
//! user-supplied TFM2 0.7.4 build. The original caster-side heal/shield power
//! detours remain; direct SDK shields need their own hook because they bypass
//! the engine shield operation. Static PE call shapes and signatures are recorded
//! in docs/research/GAME_BUILD_VERIFICATION.json; in-game integration is untested.

use std::sync::atomic::{AtomicU64, Ordering};

use super::{install_detour_generic, install_retry_due, safe_read_u64};
use crate::heal_shield_power;

// Hook status stays visible in release builds, independently of disabled debug logging.
fn status(message: String) {
    use std::io::Write;
    static LOG: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);
    let Ok(mut log) = LOG.lock() else {
        return;
    };
    if log.is_none() {
        *log = std::fs::File::create(crate::config::mod_dir().join("native-hook-status.log")).ok();
    }
    if let Some(file) = log.as_mut() {
        let _ = writeln!(file, "{message}");
    }
}

pub(crate) fn immortality_status(message: impl FnOnce() -> String) {
    static EVENTS: AtomicU64 = AtomicU64::new(0);
    if !cfg!(test) && EVENTS.fetch_add(1, Ordering::Relaxed) < 256 {
        status(message());
    }
}

// Off switch for the native equipment detours.
const ENABLED: bool = true;

// The heal: (game, world, counted, healer, target, amount, ..). It applies the
// target's healing reduction to `amount`, caps it at missing health and adds it.
const HEAL_RVA: usize = 0x1bd3fc0; // TFM2 0.7.4.
const HEAL_ORIG_LEN: usize = 12;
// push rbp, r15, r14, r13, r12, rsi, rdi, rbx
const HEAL_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];

// The add-shield: (unit, &shield). It drops a zero shield or one on a unit
// with `shield_gain_blocked` or `gladiator_duel`, else pushes the record onto
// the unit's layers.
const SHIELD_RVA: usize = 0x1bd26d0; // TFM2 0.7.4.
const SHIELD_ORIG_LEN: usize = 13;
// push rbp, rsi, rdi, rbx; sub rsp,0x48; lea rbp,[rsp+0x40]
const SHIELD_PROLOGUE: [u8; 13] = [
    0x55, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x48, 0x48, 0x8d, 0x6c, 0x24, 0x40,
];

// Verified from the 0.7.4 executable's SimVtableV1 slot 72,
// in the table at RVA 0x4385b90. It directly pushes a shield layer, bypassing SHIELD_RVA.
const SDK_SHIELD_RVA: usize = 0x3510100; // TFM2 0.7.4.
const SDK_SHIELD_PROLOGUE: [u8; 12] = [
    0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x60, 0x48, 0x8d, 0x6c, 0x24, 0x60,
];

// Entity::apply_damage in the supplied 0.7.4 PE: called at 0x19e5528
// after attacker/defender callbacks and before shield consumption / HP subtraction.
const LANDING_RVA: usize = 0x1be1110; // TFM2 0.7.4.
const LANDING_PROLOGUE: [u8; 12] = HEAL_PROLOGUE;

// Verified five-argument integer resolver: (attacker, target, amount, DamageType,
// AttackType) -> mitigated amount. Keep the original calculation, then adjust
// its defense denominator once for MLBB's separate flat/unique penetration.
const MITIGATION_RVA: usize = 0x1cfcfb0; // The copy called by the live damage routine at 0x19e47c0.
const MITIGATION_PROLOGUE: [u8; 13] = [
    0x48, 0x83, 0xec, 0x28, 0x49, 0x89, 0xd2, 0x8b, 0x44, 0x24, 0x50, 0x85, 0xc0,
];
static MITIGATION_TRAMP: AtomicU64 = AtomicU64::new(0);
static MITIGATION_INSTALLED: AtomicU64 = AtomicU64::new(0);
pub(crate) fn penetration_installed() -> bool {
    MITIGATION_INSTALLED.load(Ordering::Acquire) == 1
}
unsafe extern "C" fn cap_mitigation(saved: *mut u64, _entry_rsp: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let Some(game) = heal_shield_power::current_game() else {
        return 0;
    };
    let attacker = *saved.add(0) as usize;
    let target = *saved.add(1) as usize;
    if recipient_of(game, attacker).is_none() || recipient_of(game, target).is_none() {
        return 0;
    }
    let kind = *saved.add(3) as u32;
    let (defence_offset, pen_offset) = match kind {
        0 => (0x628, 0x410),
        1 => (0x630, 0x418),
        _ => return 0,
    };
    let Some(token) = read(attacker + 0x5b8) else {
        return 0;
    };
    let Some(defence) = read(target + defence_offset) else {
        return 0;
    };
    let Some(percent) = read(attacker + pen_offset) else {
        return 0;
    };
    let multiplier =
        crate::mlbb::rules::penetration_multiplier(game, token, defence, percent, kind);
    if multiplier <= 1. {
        return 0;
    }
    let original = MITIGATION_TRAMP.load(Ordering::Acquire) as usize;
    if original == 0 {
        return 0;
    }
    let Some(attack_type) = read((*saved.add(4) as usize).saturating_add(40)) else {
        return 0;
    };
    type Resolver = unsafe extern "C" fn(usize, usize, usize, u32, u32) -> usize;
    let original: Resolver = std::mem::transmute(original);
    let result = original(
        attacker,
        target,
        *saved.add(2) as usize,
        kind,
        attack_type as u32,
    );
    // install_replace_buy's HANDLED path restores RAX from saved slot 6.
    *saved.add(6) = (result as f64 * multiplier).round() as u64;
    1
}

fn install_penetration() {
    if penetration_installed() {
        return;
    }
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if MITIGATION_INSTALLED.load(Ordering::Relaxed) == 2 && !install_retry_due(&RETRY) {
        return;
    }
    let result = unsafe {
        (|| -> Result<(), &'static str> {
            let address = super::exe_base_addr() + MITIGATION_RVA;
            let mut bytes = Vec::new();
            if !super::safe_read_bytes(address, MITIGATION_PROLOGUE.len(), &mut bytes)
                || bytes != MITIGATION_PROLOGUE
            {
                return Err("penetration resolver signature mismatch");
            }
            // This trampoline preserves RAX and the TEST flags across its jump back.
            let stub = super::VirtualAlloc(0, 64, 0x3000, 0x40);
            if stub == 0 {
                return Err("VirtualAlloc");
            }
            let mut code = MITIGATION_PROLOGUE.to_vec();
            code.extend_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
            code.extend_from_slice(&(address + MITIGATION_PROLOGUE.len()).to_le_bytes());
            std::ptr::copy_nonoverlapping(code.as_ptr(), stub as *mut u8, code.len());
            MITIGATION_TRAMP.store(stub as u64, Ordering::Release);
            super::install_replace_buy(
                MITIGATION_RVA,
                MITIGATION_PROLOGUE.len(),
                cap_mitigation as *const () as usize,
            )?;
            Ok(())
        })()
    };
    MITIGATION_INSTALLED.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
    status(format!(
        "MLBB penetration resolver @ {MITIGATION_RVA:#x}: {result:?}"
    ));
}

pub(crate) fn entity_token(game: usize, entity: usize) -> Option<usize> {
    unsafe {
        let units = read(game.checked_add(O_GAME_ENTITIES)?)?;
        let count = read(game.checked_add(O_GAME_ENTITY_COUNT)?)?;
        if entity >= count || count > 65536 {
            return None;
        }
        read(
            units
                .checked_add(entity.checked_mul(ENTITY_SIZE)?)?
                .checked_add(0x5b8)?,
        )
    }
}

unsafe fn recipient_of(game: usize, unit: usize) -> Option<usize> {
    let units = read(game.checked_add(O_GAME_ENTITIES)?)?;
    let count = read(game.checked_add(O_GAME_ENTITY_COUNT)?)?;
    if count > 65536 {
        return None;
    }
    let offset = unit.checked_sub(units)?;
    if offset % ENTITY_SIZE != 0 || offset / ENTITY_SIZE >= count {
        return None;
    }
    Some(offset / ENTITY_SIZE)
}

unsafe extern "C" fn cap_landing(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let Some(game) = heal_shield_power::current_game() else {
        return 0;
    };
    let Some(target) = recipient_of(game, *saved.add(0) as usize) else {
        return 0;
    };
    let source_record = *saved.add(2) as usize;
    let source_token = if source_record == 0 {
        None
    } else {
        read(source_record)
    };
    // (unit, tick, &source, DamageType, AttackType, amount, ignore_shield, crit, events).
    let amount = saved.add(SAVED_ARG6);
    *amount = crate::mlbb::rules::landing_damage(
        game,
        target,
        source_token,
        *amount as usize,
        *saved.add(3) as u32,
    ) as u64;
    let unit = *saved.add(0) as usize;
    if let Some(hp) = read(unit + 0x668) {
        let ignore_shield = *saved.add(SAVED_ARG7) as u8 != 0;
        if let Some(shield) = if ignore_shield {
            Some(0)
        } else {
            shield_total(unit)
        } {
            *amount = crate::role_gear::landing(game,target,source_token,*amount as usize,hp,shield) as u64;
            let tick = *saved.add(1) as usize;
            let nonlethal = read(unit + 0x480).map(|flag| flag as u8 != 0);
            if nonlethal == Some(false) {
                *amount = crate::mlbb::rules::note_immortal_hit(
                    game, target, tick, hp, shield, *amount as usize,
                ) as u64;
            }
            if let Some(token) = source_token.filter(|_| {
                read(source_record + 8).is_some_and(|owner| owner != usize::MAX)
            }) {
                // ApplyDamage keeps this target at one HP while its nonlethal
                // flag is set. The amount above already includes mod policies.
                if let Some(nonlethal) = nonlethal {
                    let hp_at_risk = hp.saturating_sub(usize::from(nonlethal));
                    crate::mlbb::rules::note_sustain_hit(
                        game,
                        token,
                        target,
                        tick,
                        *saved.add(SAVED_ARG5) as u32,
                        *amount as usize,
                        hp_at_risk,
                        shield,
                    );
                }
            }
        }
    }
    0
}

unsafe fn shield_total(unit: usize) -> Option<usize> {
    let len = read(unit.checked_add(0x268)?)?;
    let cap = read(unit.checked_add(0x258)?)?;
    let ptr = read(unit.checked_add(0x260)?)?;
    if len > cap || len > 1024 {
        return None;
    }
    let mut total = 0usize;
    for i in 0..len {
        total = total.saturating_add(read(
            ptr.checked_add(i.checked_mul(0x38)?)?.checked_add(0x28)?,
        )?);
    }
    Some(total)
}

pub(crate) fn landing_installed() -> bool {
    LANDING_INSTALLED.load(Ordering::Relaxed) == 1
}

pub(crate) fn sdk_shield_installed() -> bool {
    SDK_SHIELD_INSTALLED.load(Ordering::Relaxed) == 1
}

// A persistent SDK shield uses a reserved remaining-duration namespace. The
// 0.7.4 updater (0x1bdc88b) decrements this u64 once per tick;
// it does not add it to a clock. Thus the high word remains an identity for
// over two years of simulation. No owner/effect-vector fields are fabricated.
pub(crate) const BLOOD_SHIELD_DURATION: usize = 0x4d4c4242_ffff_ffff;
const BLOOD_SHIELD_MASK: usize = 0xffff_ffff_0000_0000;

pub(crate) fn blood_shield(game: usize, entity: usize, clear: bool) -> Option<usize> {
    if !sdk_shield_installed() {
        return None;
    }
    unsafe { blood_shield_layers(game, entity, clear.then_some(0)).map(|(amount, _)| amount) }
}
pub(crate) fn refill_blood_shield(game: usize, entity: usize, raw: usize) -> Option<bool> {
    if !sdk_shield_installed() {
        return None;
    }
    let amount = crate::mlbb::rules::recipient_gain(game, entity, raw, true);
    unsafe { blood_shield_layers(game, entity, Some(amount)).map(|(_, count)| count > 0) }
}
unsafe fn blood_shield_layers(
    game: usize,
    entity: usize,
    replace: Option<usize>,
) -> Option<(usize, usize)> {
    let units = read(game.checked_add(O_GAME_ENTITIES)?)?;
    let count = read(game.checked_add(O_GAME_ENTITY_COUNT)?)?;
    if entity >= count || count > 65536 {
        return None;
    }
    let unit = units.checked_add(entity.checked_mul(ENTITY_SIZE)?)?;
    let cap = read(unit.checked_add(0x258)?)?;
    let layers = read(unit.checked_add(0x260)?)?;
    let len = read(unit.checked_add(0x268)?)?;
    if len > cap || len > 1024 {
        return None;
    }
    let mut total = 0usize;
    let mut found = 0usize;
    for i in 0..len {
        let layer = layers.checked_add(i.checked_mul(0x38)?)?;
        let duration = read(layer.checked_add(0x30)?)?;
        // SDK shields have neither a native owner nor break-effect records.
        if duration & BLOOD_SHIELD_MASK != BLOOD_SHIELD_DURATION & BLOOD_SHIELD_MASK
            || read(layer)? != 0
            || read(layer.checked_add(0x20)?)? != 0
        {
            continue;
        }
        let amount_addr = layer.checked_add(0x28)?;
        let amount = read(amount_addr)?;
        total = total.saturating_add(amount);
        if let Some(replacement) = replace {
            if !super::writable(amount_addr, 8) {
                return None;
            }
            // Zero lets the engine prune/drop its record next tick; a positive
            // value refills this layer. Never clear unrelated shields or move a Vec.
            std::ptr::write_unaligned(
                amount_addr as *mut u64,
                if found == 0 { replacement as u64 } else { 0 },
            );
        }
        found += 1;
    }
    Some((total, found))
}

unsafe extern "C" fn cap_sdk_shield(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let state = *saved.add(SAVED_RCX) as usize;
    let encoded = *saved.add(SAVED_RDX) as usize;
    if state == 0 || encoded == 0 {
        return 0;
    }
    if let Some(game) = read(state) {
        let amount = saved.add(2); // R8: amount; R9: duration.
        *amount =
            crate::mlbb::rules::recipient_gain(game, encoded - 1, *amount as usize, true) as u64;
    }
    0
}

// Where the capture stub saved the entry state ([`install_detour_generic`]):
// rcx, rdx, r8, r9 first, the return address at 10, then the caller's shadow
// space and its stack arguments from 15 on, written back when the stub returns.
const SAVED_RCX: usize = 0;
const SAVED_RDX: usize = 1;
const SAVED_R9: usize = 3;
const SAVED_RETURN: usize = 10;
const SAVED_ARG5: usize = 15;
const SAVED_ARG6: usize = 16;
const SAVED_ARG7: usize = 17;

// What the heal's callers tell apart (0.7.0 through 0.7.4). The byte argument is 1 on
// the engine's regeneration: a unit's HP regen and the in-base heal (both in
// its unit update, `entity.rs`) and the once-a-second heal-over-time effect,
// each a unit healing itself; it is 0 on every cast heal, the host's included.
// Giant's Horn Shard's once-a-second regen (`giants_horn_shard.rs`) is the one
// regeneration that passes 0, so it is told apart by where it returns to.
// Lifesteal and omnivamp are the damage function's one heal, the attacker on
// itself for its vamp share of the damage dealt; likewise by return address.
const REGEN_FLAG: u8 = 1;
const REGEN_RETURN_RVAS: [usize; 1] = [0x163c979]; // TFM2 0.7.4.
const VAMP_RETURN_RVA: usize = 0x19e57d7; // TFM2 0.7.4.

// The game object (0.7.0-beta2 through 0.7.4).
// Units, `ENTITY_SIZE` bytes each, and their count.
const O_GAME_ENTITIES: usize = 0x730;
const O_GAME_ENTITY_COUNT: usize = 0x738;
const ENTITY_SIZE: usize = 0x6b8;
// The players, a pointer each, and beside them what a shield names its caster
// by, one per player (`Option` at the record's start, from the world's 0x160
// slot, which finds the player whose champion the caster is).
const O_GAME_PLAYERS: usize = 0x850;
const O_GAME_PLAYER_COUNT: usize = 0x858;
const O_GAME_SHIELD_OWNERS: usize = 0x880;
const O_GAME_SHIELD_OWNER_COUNT: usize = 0x888;
// A player's champion: `Option<unit id>`.
const O_PLAYER_HAS_CHAMPION: usize = 0x8b8;
const O_PLAYER_CHAMPION: usize = 0x8c0;
// A shield record: `Option<owner>` at 0 / 8, then the amount.
const O_SHIELD_HAS_OWNER: usize = 0x0;
const O_SHIELD_OWNER: usize = 0x8;
const O_SHIELD_AMOUNT: usize = 0x28;

// A heal a carrier casts, on another unit or on itself, raised. Not the
// engine's regeneration ([`REGEN_FLAG`], [`REGEN_RETURN_RVAS`]), lifesteal or
// omnivamp ([`VAMP_RETURN_RVA`]), nor a heal of the mod's of that kind
// (`heal_shield_power::unraised`). `game` is the heal's own first argument, so
// the healer's total is the right match's.
unsafe extern "C" fn cap_heal(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let game = *saved.add(SAVED_RCX) as usize;
    let healer = *saved.add(SAVED_R9) as usize;
    let amount = saved.add(SAVED_ARG6);
    let returns_to = (*saved.add(SAVED_RETURN) as usize).wrapping_sub(super::exe_base_addr());
    if *amount == 0 {
        return 0;
    }
    // Recipient-side Oracle applies to cast heals, regeneration and vamp.
    // The engine separately applies heal_reduce, so Lifebane is not multiplied twice.
    let recipient = *saved.add(SAVED_ARG5) as usize;
    crate::mlbb::rules::note_aid(game, healer, recipient);
    *amount = crate::mlbb::rules::recipient_gain(game, recipient, *amount as usize, false) as u64;
    if *saved.add(SAVED_ARG7) as u8 == REGEN_FLAG
        || REGEN_RETURN_RVAS.contains(&returns_to)
        || returns_to == VAMP_RETURN_RVA
        || heal_shield_power::is_unraised()
    {
        return 0;
    }
    let percent = heal_shield_power::power(game, healer);
    if percent > 0.0 {
        *amount = heal_shield_power::raised(*amount as usize, percent) as u64;
    }
    0
}

// A shield a carrier casts, on another unit or on itself, raised.
unsafe extern "C" fn cap_shield(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let unit = *saved.add(SAVED_RCX) as usize;
    let record = *saved.add(SAVED_RDX) as usize;
    if let Some(game) = heal_shield_power::current_game() {
        raise_shield(game, unit, record);
    }
    0
}

unsafe fn raise_shield(game: usize, unit: usize, record: usize) -> Option<()> {
    // The unit must be one of this match's: the add-shield is handed no game,
    // so the match is the one this thread last ticked.
    let recipient = recipient_of(game, unit)?;
    let mut amount = read(record + O_SHIELD_AMOUNT)?;
    if read(record + O_SHIELD_HAS_OWNER)? == 1 {
        if let Some(caster) = caster_champion(game, read(record + O_SHIELD_OWNER)?) {
            if amount > 0 {
                crate::mlbb::rules::note_aid(game, caster, recipient);
            }
            let percent = heal_shield_power::power(game, caster);
            if percent > 0. {
                amount = heal_shield_power::raised(amount, percent);
            }
        }
    }
    amount = crate::mlbb::rules::recipient_gain(game, recipient, amount, true);
    *((record + O_SHIELD_AMOUNT) as *mut usize) = amount;
    Some(())
}

// The champion of the player a shield names as its owner.
unsafe fn caster_champion(game: usize, owner: usize) -> Option<usize> {
    let players = read(game + O_GAME_PLAYERS)?;
    let owners = read(game + O_GAME_SHIELD_OWNERS)?;
    let count = read(game + O_GAME_PLAYER_COUNT)?.min(read(game + O_GAME_SHIELD_OWNER_COUNT)?);
    if count > 64 {
        return None;
    }
    for index in 0..count {
        if read(owners + index * 8)? != owner {
            continue;
        }
        let player = read(players + index * 8)?;
        if read(player + O_PLAYER_HAS_CHAMPION)? == 0 {
            return None;
        }
        return read(player + O_PLAYER_CHAMPION);
    }
    None
}

unsafe fn read(addr: usize) -> Option<usize> {
    safe_read_u64(addr).map(|value| value as usize)
}

static HEAL_INSTALLED: AtomicU64 = AtomicU64::new(0);
static SHIELD_INSTALLED: AtomicU64 = AtomicU64::new(0);
static SDK_SHIELD_INSTALLED: AtomicU64 = AtomicU64::new(0);
static LANDING_INSTALLED: AtomicU64 = AtomicU64::new(0);

// Installs both detours, from wherever the other hooks of this half install;
// each retries on its own after a failure ([`install_retry_due`]).
pub(super) fn install() {
    if !ENABLED {
        return;
    }
    install_penetration();
    static HEAL_RETRY: AtomicU64 = AtomicU64::new(0);
    static SHIELD_RETRY: AtomicU64 = AtomicU64::new(0);
    static SDK_SHIELD_RETRY: AtomicU64 = AtomicU64::new(0);
    static LANDING_RETRY: AtomicU64 = AtomicU64::new(0);
    install_one(
        "typed damage landing",
        &LANDING_INSTALLED,
        &LANDING_RETRY,
        LANDING_RVA,
        LANDING_PROLOGUE.len(),
        &LANDING_PROLOGUE,
        cap_landing as *const () as usize,
    );
    install_one(
        "SDK add-shield",
        &SDK_SHIELD_INSTALLED,
        &SDK_SHIELD_RETRY,
        SDK_SHIELD_RVA,
        SDK_SHIELD_PROLOGUE.len(),
        &SDK_SHIELD_PROLOGUE,
        cap_sdk_shield as *const () as usize,
    );
    install_one(
        "heal",
        &HEAL_INSTALLED,
        &HEAL_RETRY,
        HEAL_RVA,
        HEAL_ORIG_LEN,
        &HEAL_PROLOGUE,
        cap_heal as *const () as usize,
    );
    install_one(
        "add-shield",
        &SHIELD_INSTALLED,
        &SHIELD_RETRY,
        SHIELD_RVA,
        SHIELD_ORIG_LEN,
        &SHIELD_PROLOGUE,
        cap_shield as *const () as usize,
    );
}

fn install_one(
    name: &str,
    installed: &AtomicU64,
    retry: &AtomicU64,
    rva: usize,
    orig_len: usize,
    prologue: &[u8],
    cap: usize,
) {
    let state = installed.load(Ordering::Relaxed);
    if state == 1 || (state == 2 && !install_retry_due(retry)) {
        return;
    }
    let r = unsafe { install_detour_generic(rva, orig_len, cap, prologue) };
    installed.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    if state == 0 || r.is_ok() {
        status(format!("heal/shield power: {name} hook @ {rva:#x}: {r:?}"));
    }
}

#[cfg(test)]
mod blood_layer_tests {
    use super::*;
    #[test]
    fn named_duration_layer_survives_vector_reordering_and_ignores_ally_shields() {
        super::super::seh_install();
        let mut game = vec![0usize; 0x900 / 8];
        let mut unit = vec![0usize; ENTITY_SIZE / 8];
        let mut layers = vec![
            [0usize, 0, 0, 8, 0, 400, 120],
            [0, 0, 0, 8, 0, 300, BLOOD_SHIELD_DURATION - 5],
            [1, 77, 0, 8, 0, 500, 999],
        ];
        game[O_GAME_ENTITIES / 8] = unit.as_mut_ptr() as usize;
        game[O_GAME_ENTITY_COUNT / 8] = 1;
        unit[0x258 / 8] = layers.len();
        unit[0x260 / 8] = layers.as_mut_ptr() as usize;
        unit[0x268 / 8] = layers.len();
        let game = game.as_mut_ptr() as usize;
        unsafe {
            assert_eq!(blood_shield_layers(game, 0, None), Some((300, 1)));
            layers.swap(0, 1);
            layers[0][5] = 100;
            assert_eq!(blood_shield_layers(game, 0, None), Some((100, 1)));
            assert_eq!(blood_shield_layers(game, 0, Some(475)), Some((100, 1)));
            assert_eq!(layers[0][5], 475);
            assert_eq!((layers[1][5], layers[2][5]), (400, 500));
            assert_eq!(blood_shield_layers(game, 0, Some(0)), Some((475, 1)));
            assert_eq!(layers[0][5], 0);
            assert_eq!((layers[1][5], layers[2][5]), (400, 500));
            assert_eq!(blood_shield_layers(game, 1, None), None);
            unit[0x268 / 8] = 1025;
            assert_eq!(blood_shield_layers(game, 0, None), None);
        }
    }
}
