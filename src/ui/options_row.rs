//! The mod's Item Rules row on the Options dialog's Competition tab: *All
//! Radiants*, *First Item Mythical* or *No Mythicals*, kept in the save and
//! applied by [`crate::item_tiers`]. It sits above the Item Patcher Settings
//! row (user, 2026-10-16), which waits for it to be appended.
//!
//! Greyed out everywhere but the management screens ([`locked_now`], user
//! 2026-10-08). A click applies at once, into the save; the dialog's OK and
//! Cancel do not touch it.
//!
//! Seen working on its own dialog finder (2026-10-08); not yet tried on the
//! shared one.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use mod_api_stable::{ClientSceneKindV1, SceneKindV1, StableClient, UiEventKindV1};

use super::options_dialog::{self, escape, text};
use crate::item_tiers::Rule;

// The row's node, and under it the box its cells are in.
pub(super) const ROW: &str = "riot_item_rules";
const CELLS_UNDER: &str = ".options.";

// The row's title: this mod's key in the game's text, and the English.
const TITLE: (&str, &str) = ("builds.item_rules", "Item Rules");

// One cell: the side it asserts, its node's name, and its label and its hint
// the way [`TITLE`] is written.
struct Cell {
    rule: Rule,
    node: &'static str,
    label: (&'static str, &'static str),
    hint: (&'static str, &'static str),
}

// The cells, left to right.
const CELLS: [Cell; 3] = [
    Cell {
        rule: Rule::AllRadiants,
        node: "all",
        label: ("builds.item_rules_all", "All Mythicals"),
        hint: (
            "builds.item_rules_all_hint",
            "Every item in a build is upgraded to its Mythical form.",
        ),
    },
    Cell {
        rule: Rule::FirstItemRadiant,
        node: "first",
        label: ("builds.item_rules_first", "First Item Mythical"),
        hint: (
            "builds.item_rules_first_hint",
            "Only the first item in a build is upgraded to Mythical. Every other item stops at Finished.",
        ),
    },
    Cell {
        rule: Rule::NoRadiants,
        node: "none",
        label: ("builds.item_rules_none", "No Mythicals"),
        hint: (
            "builds.item_rules_none_hint",
            "Every item in a build stops at Finished. Nothing is upgraded to Mythical.",
        ),
    },
];
// Both cells' hint while the row is greyed out.
const LOCKED_HINT: (&str, &str) = (
    "builds.item_rules_locked",
    "Item Rules can only be changed between matches, not during a draft or a match.",
);

// Frames between two of the test log's looks at the dialog.
const LOG_EVERY: u32 = 20;

// Frames [`sync`] has run.
static FRAME: AtomicU32 = AtomicU32::new(0);

// Whether the dialog was up on the last frame.
static UP: AtomicBool = AtomicBool::new(false);

// What the row was last painted as ([`look`]): 0 for not yet.
static PAINTED: AtomicU8 = AtomicU8::new(0);

// Whether the setting may not be changed now. Worked out on the frame
// ([`locked_now`]) and kept here for the click handler, which is not a
// place the client can be asked what screen it is on.
static LOCKED: AtomicBool = AtomicBool::new(false);

// The `options.` lines of `match-builds.log` when its switch is on
// (`crate::match_builds::log`): the tab, where the row landed, and every cell
// event.
fn log(key: &str, text: impl FnOnce() -> String) {
    crate::match_builds::log(&format!("options.{key}"), text);
}

// The row, shaped like the layout's own three-cell rows: 846 pixels inside
// the box's padding, 282 a cell.
fn row_source(ctx: &StableClient<'_>) -> String {
    let cells: String = CELLS
        .iter()
        .map(|cell| {
            format!(
                "#{}:color_selectable {{\n\
                 @\"asset/base/style/main#strategy_option\";\n\
                 width: 282px;\n\
                 height: 32px;\n\
                 label: {{ size: 16; }}\n\
                 selected_label: {{ size: 16; }}\n\
                 text: \"{}\";\n\
                 hint: \"{}\";\n\
                 }}\n",
                cell.node,
                escape(&text(ctx, cell.label)),
                escape(&text(ctx, cell.hint)),
            )
        })
        .collect();
    format!(
        "{ROW}:empty {{\n\
         width: 1160px;\n\
         height: 40px;\n\
         \n\
         #header:label {{\n\
         @\"asset/base/style/main#label\";\n\
         width: 305px;\n\
         height: 40px;\n\
         align_y: Center;\n\
         size: 16;\n\
         text: \"{title}\";\n\
         }}\n\
         \n\
         #options:color {{\n\
         x: 255px;\n\
         width: 852px;\n\
         height: 40px;\n\
         padding: {{ left: 3px; right: 3px; top: 4px; bottom: 4px; }}\n\
         color: #4a4c56ff;\n\
         stroke: 1;\n\
         back_color: #00000000;\n\
         child_type: LeftToRight {{ spacing: 0px; }}\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         {cells}\
         }}\n\
         }}\n",
        title = escape(&text(ctx, TITLE)),
    )
}

// The row's look as one number: which side is the setting, and whether it
// is greyed out. Never 0.
fn look(rule: Rule, locked: bool) -> u8 {
    1 + rule.code() + 4 * locked as u8
}

// Lights the cell of the current setting in the row at `row`, greyed out while
// locked, with the matching hint. Painted, not set: see
// `strategy_ui::paint_toggle`.
fn paint(ctx: &mut StableClient<'_>, row: &str) {
    let rule = crate::item_tiers::rule();
    let locked = LOCKED.load(Ordering::Relaxed);
    let size = if row.contains(".footer.") { 11 } else { 16 };
    for cell in &CELLS {
        let lit = cell.rule == rule;
        let style = if locked {
            crate::strategy_ui::toggle_off_style_sized(lit, size)
        } else {
            crate::strategy_ui::toggle_style_sized(lit, size)
        };
        let hint = escape(&text(ctx, if locked { LOCKED_HINT } else { cell.hint }));
        let path = format!("{row}{CELLS_UNDER}{}", cell.node);
        ctx.ui_set_properties(&path, &style);
        ctx.ui_set_properties(&path, &format!("hint: \"{hint}\";"));
    }
}

// Whether the setting may not be changed now: anywhere but the management
// screens. A host that cannot say which screen is up counts as management, so
// the setting never locks for good.
pub(super) fn locked_now(ctx: &StableClient<'_>) -> bool {
    match ctx.client_scene_kind() {
        Some(ClientSceneKindV1::Main) => false,
        Some(_) => true,
        None => ctx.scene_kind() == Some(SceneKindV1::GameTest),
    }
}

// Registers the click handler on one cell, every time the row spawns or the
// dialog opens: registering once lost clicks after the pause menu was reopened
// (2026-10-08). A surviving old handler only makes a click assert its value
// twice.
fn register(ctx: &mut StableClient<'_>, path: &str) {
    let registered = ctx.ui_register_path_events(path, handle_event);
    log("register", || format!("`{path}` registered={registered}"));
}

// A click on a cell asserts that cell's side.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    log("event", || format!("{:?} on `{}`", event.kind, event.path));
    // An unreported kind counts as a click, the gate `item_stats::ui` uses.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    let Some((row, node)) = event.path.rsplit_once(CELLS_UNDER) else {
        return;
    };
    let Some(cell) = CELLS.iter().find(|cell| cell.node == node) else {
        return;
    };
    if !row.ends_with(ROW) {
        return;
    }
    // Greyed out: the click changes nothing, and the row is painted again
    // in case the click itself changed how the cell looks.
    if LOCKED.load(Ordering::Relaxed) {
        paint(ctx, row);
        return;
    }
    if crate::item_tiers::rule() != cell.rule {
        crate::item_tiers::choose(cell.rule);
        log("click", || format!("{:?}", cell.rule));
    }
    paint(ctx, row);
}

// Keeps the row in the dialog and on its tab. Every client frame; with the
// dialog closed this is one lookup.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    let Some(dialog) = options_dialog::showing(ctx, frame) else {
        UP.store(false, Ordering::Relaxed);
        return;
    };
    let opened = !UP.swap(true, Ordering::Relaxed);

    let contents = format!("{dialog}.contents");
    let row = format!("{contents}.{ROW}");
    // Spawned once each time game code builds the dialog, after its own rows
    // and before the patcher row, which waits for this one.
    let fresh = !ctx.ui_exists(&row);
    if fresh {
        let source = row_source(ctx);
        let spawned = ctx.ui_spawn_source(&contents, &source);
        if !spawned {
            log("row", || format!("could not be spawned under `{contents}`"));
            return;
        }
        paint(ctx, &row);
    }
    // On a fresh row and each time the dialog opens, since what ends a
    // registration is not known (see `register`).
    if fresh || opened {
        for cell in &CELLS {
            register(ctx, &format!("{row}{CELLS_UNDER}{}", cell.node));
        }
    }
    // The setting can change under an open dialog, when the save is read,
    // and so can whether it may be changed: the row says what rules now.
    let locked = locked_now(ctx);
    LOCKED.store(locked, Ordering::Relaxed);
    if PAINTED.load(Ordering::Relaxed) != look(crate::item_tiers::rule(), locked) {
        paint(ctx, &row);
        PAINTED.store(look(crate::item_tiers::rule(), locked), Ordering::Relaxed);
    }

    let competition = options_dialog::competition_up(ctx, &dialog);
    ctx.ui_set_visible(&row, competition);
    super::first_slot_options::sync(ctx,&contents,false,competition);
    if opened || frame % LOG_EVERY == 0 {
        log("dialog", || {
            format!(
                "`{dialog}` up; screen={:?} of {:?} -> locked={locked}; competition tab selected={:?} -> row shown={competition}; contents holds {:?}; the dialog holds {:?}; row at {:?}",
                ctx.client_scene_kind(),
                ctx.scene_kind(),
                ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition")),
                ctx.ui_child_names(&contents),
                ctx.ui_child_names(&dialog),
                ctx.ui_node_rect(&row),
            )
        });
    }
}

// Same saved rule and handler as Options, exposed beside the normal Builds
// footer controls. Narrow training panels keep the existing Options control.
pub(crate) fn sync_editor(ctx: &mut StableClient<'_>, footer: &str, width: u32) {
    use std::sync::Mutex;
    static LAST: Mutex<Option<(String, u8)>> = Mutex::new(None);
    super::first_slot_options::sync(ctx,footer,true,width >= 755);

    let row = format!("{footer}.{ROW}");
    let locked = locked_now(ctx);
    LOCKED.store(locked, Ordering::Relaxed);
    if width < 1340 {
        ctx.ui_set_visible(&row, false);
        return;
    }
    let fresh = !ctx.ui_exists(&row);
    if fresh {
        let cells:String=CELLS.iter().map(|cell|format!(
            "#{}:color_selectable {{ @\"asset/base/style/main#strategy_option\"; width: 116px; height: 32px; label: {{size: 12;align_y:Center;fit_width:true;}} selected_label: {{size: 12;align_y:Center;fit_width:true;}} text: \"{}\"; hint: \"{}\"; }}",
            cell.node,escape(&text(ctx,cell.label)),escape(&text(ctx,cell.hint)))).collect();
        let source=format!("{ROW}:color {{ x: 755px; y: 10px; width: 365px; height: 40px; color: #4a4c56ff; stroke:1; back_color:#00000000; rounding:Uniform{{rounding:8;}} padding: {{left: 4px; right: 4px; top: 4px; bottom: 4px;}} #options:empty {{ width: 357px; height: 32px; child_type: LeftToRight {{ spacing: 4px; }} {cells} }} }}");
        if !ctx.ui_spawn_source(footer, &source) {
            return;
        }
        for cell in &CELLS {
            register(ctx, &format!("{row}{CELLS_UNDER}{}", cell.node));
        }
    }
    ctx.ui_set_visible(&row, true);
    let current = look(crate::item_tiers::rule(), locked);
    let changed = LAST
        .lock()
        .map(|mut last| {
            let changed = fresh || last.as_ref() != Some(&(row.clone(), current));
            *last = Some((row.clone(), current));
            changed
        })
        .unwrap_or(true);
    if changed {
        paint(ctx, &row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api_stable::{ClientCtxV1, UiClickHandlerFn, UiVtableV1};
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::c_void,
        mem::size_of,
    };
    #[derive(Default)]
    struct Host {
        nodes: BTreeSet<String>,
        sources: Vec<String>,
        callbacks: BTreeMap<String, (UiClickHandlerFn, *mut c_void)>,
        event: String,
        visible: BTreeMap<String, bool>,
    }
    unsafe fn host<'a>(s: *const c_void) -> &'a mut Host {
        &mut *(s as *mut Host)
    }
    unsafe fn string(p: *const u8, n: usize) -> String {
        String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned()
    }
    unsafe extern "C" fn exists(s: *const c_void, p: *const u8, n: usize) -> bool {
        host(s).nodes.contains(&string(p, n))
    }
    unsafe extern "C" fn visible(s: *mut c_void, p: *const u8, n: usize, v: bool) -> bool {
        host(s).visible.insert(string(p, n), v);
        true
    }
    unsafe extern "C" fn spawn(
        s: *mut c_void,
        p: *const u8,
        n: usize,
        source: *const u8,
        len: usize,
    ) -> bool {
        let h = host(s);
        let src=string(source,len);
        let node=src.split(':').next().unwrap().trim();
        h.nodes.insert(format!("{}.{}", string(p, n),node));
        h.sources.push(src);
        true
    }
    unsafe extern "C" fn register(
        s: *mut c_void,
        p: *const u8,
        n: usize,
        cb: Option<UiClickHandlerFn>,
        u: *mut c_void,
    ) -> bool {
        if let Some(cb) = cb {
            host(s).callbacks.insert(string(p, n), (cb, u));
        }
        true
    }
    unsafe extern "C" fn properties(
        _s: *mut c_void,
        _p: *const u8,
        _n: usize,
        _source: *const u8,
        _len: usize,
    ) -> bool {
        true
    }
    unsafe extern "C" fn event(
        s: *const c_void,
        k: *mut u32,
        p: *mut u8,
        cap: usize,
        n: *mut usize,
        _payload: *mut u8,
        _pcap: usize,
        pn: *mut usize,
    ) -> bool {
        let h = host(s);
        *k = UiEventKindV1::Click.code();
        *n = h.event.len();
        *pn = 0;
        if !p.is_null() && cap >= *n {
            std::ptr::copy_nonoverlapping(h.event.as_ptr(), p, *n);
        }
        true
    }
    #[test]
    fn editor_buttons_use_the_saved_item_rule_handler_and_respect_the_match_lock() {
        let previous = crate::item_tiers::rule();
        let mut h = Host::default();
        let mut ui: UiVtableV1 = unsafe { std::mem::zeroed() };
        ui.size = size_of::<UiVtableV1>();
        ui.query_exists = Some(exists);
        ui.set_visible = Some(visible);
        ui.spawn_source = Some(spawn);
        ui.register_path_events = Some(register);
        ui.current_event = Some(event);
        ui.set_properties = Some(properties);
        let mut raw: ClientCtxV1 = unsafe { std::mem::zeroed() };
        raw.size = size_of::<ClientCtxV1>();
        raw.ui = &ui;
        raw.state = (&mut h as *mut Host).cast();
        {
            let mut ctx = unsafe { StableClient::from_raw(&mut raw).unwrap() };
            sync_editor(&mut ctx, "builds.popup.footer", 1364);
        }
        assert!(h.sources.iter().any(|s|s.contains("No Mythicals")));
        assert!(h.sources.iter().any(|s|s.contains("Roam Boots First")));
        assert_eq!(h.callbacks.len(), 5);
        h.event = format!("builds.popup.footer.{ROW}.options.none");
        let (cb, u) = h.callbacks[&h.event];
        unsafe {
            cb(u, &mut raw);
        }
        assert_eq!(crate::item_tiers::rule(), Rule::NoRadiants);
        LOCKED.store(true, Ordering::Relaxed);
        h.event = format!("builds.popup.footer.{ROW}.options.all");
        let (cb, u) = h.callbacks[&h.event];
        unsafe {
            cb(u, &mut raw);
        }
        assert_eq!(crate::item_tiers::rule(), Rule::NoRadiants);
        {
            let mut ctx = unsafe { StableClient::from_raw(&mut raw).unwrap() };
            sync_editor(&mut ctx, "builds.popup.footer", 1100);
        }
        assert_eq!(h.visible[&format!("builds.popup.footer.{ROW}")], false);
        crate::item_tiers::choose(previous);
        LOCKED.store(false, Ordering::Relaxed);
    }
}
