//! Per-champion roaming presentation. Shared boot definitions remain unchanged.
use super::{item_counts, item_spend, match_builds};
use crate::role_gear::{self, Roam, RoamStatus};
use mod_api_stable::StableClient;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};
const TIP: &str = "ingame.item_tooltip";
const DESC: &str = "ingame.item_tooltip.data.desc";
const BADGE: &str = "mlbb_roam_badge";
static SHOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());
static HAD_TIP: Mutex<bool> = Mutex::new(false);

pub(super) fn badge_source(roam: Roam, size: usize) -> Option<String> {
    let color = role_gear::badge_colour(roam)?;
    Some(format!(
        r##"{BADGE}:color {{
      palette: false; ignore_event: true;
      anchor_x: 1; pivot_x: 1; x: 0px; y: 0px;
      width: {size}px; height: {size}px; color: #141722ff;
      rounding: Uniform {{ rounding: 3; }}
      #icon:image {{ ignore_event: true; x: 2px; y: 2px;
        width: {glyph}px; height: {glyph}px;
        source: "asset/base/ui/banpick/champion_stat_icon#sheet";
        rect_tag: "speed_0"; color: {color};
      }}
    }}"##,
        glyph = size.saturating_sub(4)
    ))
}
fn ensure_badge(
    ctx: &mut StableClient<'_>,
    parent: &str,
    roam: Roam,
    size: usize,
) -> Option<String> {
    let source = badge_source(roam, size)?;
    let path = format!("{parent}.{BADGE}");
    if !ctx.ui_exists(&path) && !ctx.ui_spawn_source(parent, &source) {
        static FAILED:std::sync::OnceLock<()>=std::sync::OnceLock::new();
        if FAILED.set(()).is_ok() {crate::hook::build_status("roaming/badge-spawn",format!("could not spawn badge under {parent}; effect={roam:?}"));}
        return None;
    }
    let color = role_gear::badge_colour(roam)?;
    ctx.ui_set_properties(&format!("{path}.icon"), &format!("color: {color};"));
    ctx.ui_set_visible(&path, true);
    Some(path)
}
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in text.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}
pub(super) fn context_option(base: &str, status: Option<&RoamStatus>, tick: usize) -> String {
    // Drop the shared catalogue note or last frame's contextual block, retaining
    // the boot's own stats/passive prefix. This is idempotent across hover frames.
    let prefix = base
        .lines()
        .take_while(|l| !plain(l).trim_start().starts_with("Roaming"))
        .collect::<Vec<_>>()
        .join("\n");
    let effect = status
        .map(|s| role_gear::selected_roam_description(s, tick))
        .unwrap_or_else(|| "Roaming: Selection unavailable.".into());
    if prefix.trim().is_empty() {
        effect
    } else {
        format!("{}\n{effect}", prefix.trim_end())
    }
}
fn grid_targets(
    ctx: &StableClient<'_>,
    seed: Option<u64>,
    tick: Option<usize>,
) -> Vec<(String, Roam, usize)> {
    let mut wanted = Vec::new();
    for (view, host) in item_spend::ROW_HOSTS {
        if ctx.ui_visible(view) != Some(true) {
            continue;
        }
        let rows = ctx.ui_child_names(host);
        for (place, row) in rows.iter().enumerate() {
            let Some(lane) = item_spend::row_lane(&rows, place) else {
                continue;
            };
            for (side, half) in item_spend::HALVES.iter().enumerate() {
                let Some(status) = role_gear::shown_roam_status(seed, side, lane, tick) else {
                    continue;
                };
                if role_gear::badge_colour(status.roam).is_none() {
                    continue;
                }
                let nodes = item_spend::slot_nodes(ctx, &format!("{host}.{row}.{half}.items"));
                for (slot, key) in &status.boot_slots {
                    if role_gear::is_completed_boot(key) {
                        if let Some(node) = nodes.get(*slot) {
                            wanted.push((node.clone(), status.roam, 16));
                        }
                    }
                }
            }
        }
    }
    wanted
}
fn sync_badges(ctx: &mut StableClient<'_>) {
    static FRAME: AtomicU32 = AtomicU32::new(0);
    if FRAME.fetch_add(1, Ordering::Relaxed) % 6 != 0 {
        return;
    }
    let seed = match_builds::shown_seed();
    let tick = item_counts::shown_tick(ctx);
    let mut wanted = grid_targets(ctx, seed, tick);
    wanted.extend(
        match_builds::planned_roam_badges(ctx)
            .into_iter()
            .map(|(path, roam)| (path, roam, 13)),
    );
    let mut paths = Vec::new();
    for (host, roam, size) in wanted {
        if let Some(path) = ensure_badge(ctx, &host, roam, size) {
            paths.push(path);
        }
    }
    let previous = SHOWN
        .lock()
        .map(|mut s| std::mem::replace(&mut *s, paths.clone()))
        .unwrap_or_default();
    for old in previous {
        if !paths.contains(&old) {
            ctx.ui_remove_node(&old);
        }
    }
}
fn restore_desc_anchor(ctx: &mut StableClient<'_>) {
    if HAD_TIP
        .lock()
        .map(|mut b| std::mem::replace(&mut *b, false))
        .unwrap_or(false)
    {
        ctx.ui_set_properties(DESC, "anchor_y: 1; pivot_y: 1; y: 0px;");
    }
    ctx.ui_remove_node(&format!("{TIP}.data.slot.{BADGE}"));
}
fn sync_native_tip(ctx: &mut StableClient<'_>) {
    if ctx.ui_visible(TIP) != Some(true) {
        restore_desc_anchor(ctx);
        return;
    }
    let Some(key) = ctx.ui_text("ingame.item_tooltip.data.name").and_then(|n| {
        crate::patches::text::texts()
            .key_of_name(&n)
            .map(str::to_string)
    }) else {
        restore_desc_anchor(ctx);
        return;
    };
    if !role_gear::is_completed_boot(&key) {
        restore_desc_anchor(ctx);
        return;
    }
    let tick = item_counts::shown_tick(ctx);
    let seed = match_builds::shown_seed();
    let status = item_spend::hovered(ctx).and_then(|(side, lane, slot)| {
        let status = role_gear::shown_roam_status(seed, side, lane?, tick)?;
        (!status.progress_known
            || status
                .boot_slots
                .iter()
                .any(|(s, k)| *s == slot && k == &key))
        .then_some(status)
    });
    let Some(base) = ctx.ui_text(DESC) else {
        return;
    };
    let text = context_option(&base, status.as_ref(), tick.unwrap_or(0));
    ctx.ui_set_text(DESC, &text);
    let lines = plain(&text)
        .lines()
        .map(|l| l.chars().count().div_ceil(30).max(1))
        .sum::<usize>();
    let body = (lines * 20).max(40);
    let height = (body + 72).min(950);
    ctx.ui_set_properties(
        DESC,
        &format!("anchor_y: 0; pivot_y: 0; y: 52px; height: {body}px;"),
    );
    ctx.ui_set_properties(TIP, &format!("height: {height}px;"));
    ctx.ui_set_properties(&format!("{TIP}.bg"), &format!("height: {}px;", height - 2));
    ctx.ui_set_properties(
        &format!("{TIP}.data"),
        &format!("height: {}px;", height - 14),
    );
    if let (Some((_, top, _, _)), Some((_, root_top, _, root_h))) =
        (ctx.ui_node_rect(TIP), ctx.ui_node_rect("ingame"))
    {
        let y = (top - root_top)
            .min((root_h - height as f32 - 8.).max(0.))
            .max(0.);
        ctx.ui_set_properties(TIP, &format!("y: {y}px;"));
    }
    if let Ok(mut b) = HAD_TIP.lock() {
        *b = true;
    }
    if let Some(s) = status {
        if ensure_badge(ctx, &format!("{TIP}.data.slot"), s.roam, 16).is_none() {
            ctx.ui_remove_node(&format!("{TIP}.data.slot.{BADGE}"));
        }
    } else {
        ctx.ui_remove_node(&format!("{TIP}.data.slot.{BADGE}"));
    }
}
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    sync_badges(ctx);
    sync_native_tip(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn status(roam: Roam) -> RoamStatus {
        RoamStatus {
            roam,
            progress_known: true,
            boots: true,
            boot_slots: vec![(0, "magic_shoes".into())],
            alive: true,
            earned: 192,
            unlock: 192,
            full_power: 480,
            ready_at: 0,
            concealed_until: 0,
        }
    }
    #[test]
    fn shared_boot_gets_only_its_holders_blessing_and_repeated_frames_are_idempotent() {
        let base = "75 HP\nRoaming: generic note.";
        let favor = context_option(base, Some(&status(Roam::Favor)), 0);
        assert!(favor.contains("Favor"));
        assert!(!favor.contains("Conceal"));
        assert!(favor.starts_with("75 HP"));
        assert_eq!(context_option(&favor, Some(&status(Roam::Favor)), 0), favor);
        let conceal = context_option(&favor, Some(&status(Roam::Conceal)), 0);
        assert!(conceal.contains("Conceal"));
        assert!(!conceal.contains("Favor"));
        assert!(conceal.starts_with("75 HP"));
    }
    #[test]
    fn badge_uses_existing_boot_glyph_and_exact_owner_requested_colours() {
        for (r, c) in [
            (Roam::Conceal, "#aa75ffff"),
            (Roam::Favor, "#65db8fff"),
            (Roam::Encourage, "#ed5c6bff"),
            (Roam::DireHit, "#65baffff"),
        ] {
            let source = badge_source(r, 16).unwrap();
            assert!(source.contains(c));
            assert!(source.contains("speed_0"));
            assert!(source.contains("ignore_event: true"));
        }
        assert!(badge_source(Roam::None, 16).is_none());
        assert!(badge_source(Roam::Auto, 16).is_none());
    }
}
