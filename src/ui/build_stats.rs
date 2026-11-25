//! Statistics-native champion slot outcomes and reviewed plan application.
use mod_api_stable::*;
use std::{collections::BTreeSet,sync::Mutex};
use crate::{build_config::{self,Role,ChampionRow},item_stats::{self,builds}};
const ROOT:&str="main.top.right.statistics";
const PANEL:&str="main.top.right.statistics.mlbb_build_stats";
#[derive(PartialEq)]
struct DrawInput {
    observations: u64,
    balance: String,
    query: String,
    language: String,
    locked: bool,
}

#[derive(Default)]
struct State {
    open: bool,
    champ: Option<String>,
    lane: usize,
    page: usize,
    patch: usize,
    tier: usize,
    details: bool,
    balance_only: bool,
    history_only: bool,
    roster: BTreeSet<String>,
    shown_champs: Vec<String>,
    roster_refresh: bool,
    preview: Option<(ChampionRow, Vec<ChampionRow>)>,
    planner: bool,
    library: Option<crate::build_library::Library>,
    library_index: usize,
    pending_import: bool,
    selected_slot: usize,
    item_page: usize,
    undo_slots: Option<Vec<Option<String>>>,
    policy: (u8, u8),
    message: String,
    painted: String,
    drawn_for: Option<DrawInput>,
}

static STATE: std::sync::LazyLock<Mutex<State>> = std::sync::LazyLock::new(|| {
    Mutex::new(State { patch: 1, ..Default::default() })
});
const CONTENT: &str = "main.top.right.statistics.mlbb_build_stats.content";
const SEARCH: &str = "main.top.right.statistics.mlbb_build_stats.champion_search";
const NAME: &str = "main.top.right.statistics.mlbb_build_stats.plan_name";
const ITEM_SEARCH: &str = "main.top.right.statistics.mlbb_build_stats.equipment_search";

fn edit_box(id: &str, x: usize, y: usize, width: usize, placeholder: &str) -> String {
    format!(r#"
    #{id}:text_edit {{
        @"asset/base/style/main#text_edit";
        visible:false; z:20; x:{x}px; y:{y}px; width:{width}px; height:36px;
        size:15; align_y:Center;
        padding:{{left:12px; right:10px; top:4px; bottom:4px;}}
        placeholder:"{placeholder}"; max_length:60;
    }}"#)
}

fn shell_source() -> String {
    let search = edit_box("champion_search", 24, 82, 385, "Search champions");
    let name = edit_box("plan_name", 36, 721, 350, "Name this alternative");
    let equipment = edit_box("equipment_search", 1168, 115, 374, "Search equipment");
    format!(r#"mlbb_build_stats:color {{
        y:52px; width:1600px; height:916px; color:#161721ff;
        rounding:Uniform{{rounding:12;}} {search}{name}{equipment}
    }}"#)
}

static EVENTS:super::Registered=super::Registered::new(event);
fn safe(s:&str)->String{s.chars().filter(|c|!matches!(c,'"'|'\\'|'{'|'}'|';'|'\n'|'\r')).collect()}
fn label(id:&str,x:usize,y:usize,w:usize,text:&str,size:usize)->String{format!("#{id}:label {{@\"asset/base/style/main#label\";ignore_event:true;x:{x}px;y:{y}px;width:{w}px;height:28px;size:{size};align_y:Center;color:#c2c6ceff;text:\"{}\";}}",safe(text))}
fn button(id: &str, x: usize, y: usize, w: usize, text: &str) -> String {
    let row = id.starts_with("champ_") || id.starts_with("pick_item_");
    let style = if id == "apply" { "primary_button" } else { "tertiary_button" };
    let text = super::button_text(&safe(text), 14, row);
    format!(r#"#{id}:color_icon_button {{@"asset/base/style/main#{style}";
        x:{x}px; y:{y}px; width:{w}px; height:36px; {text} }}"#)
}

fn selection_button(id: &str, x: usize, y: usize, width: usize, text: &str, selected: bool) -> String {
    let label = safe(text);
    let style = super::strategy_ui::toggle_style_sized(selected, 14);
    format!(r#"#{id}:color_selectable {{@"asset/base/style/main#strategy_option";
        x:{x}px; y:{y}px; width:{width}px; height:36px; text:"{label}"; {style} }}"#)
}

fn rate(c:builds::Count)->String{c.rate().map(|r|format!("{r:.1}%")).unwrap_or_else(||"—".into())}
fn rate_label(id:&str,x:usize,y:usize,w:usize,text:&str,size:usize,count:builds::Count)->String{
 let color=if count.games<10{"#8a8c96ff"}else if count.rate().unwrap_or(50.)>=55.{"#2fb35aff"}else if count.rate().unwrap_or(50.)<45.{"#e0483eff"}else{"#65baffff"};
 label(id,x,y,w,text,size).replace("color:#c2c6ceff",&format!("color:{color}"))
}
fn short_name(name:&str)->String{name.strip_prefix("Mythical ").map(|n|format!("M. {n}")).unwrap_or_else(||name.into())}
fn cname(ctx: &StableClient<'_>, key: &str) -> String {
    super::strategy_ui::champion_display_name(ctx, &crate::patches::text::game_language(), key)
}

fn champion_rows(state: &State, table: &builds::Table) -> BTreeSet<String> {
    if state.history_only {
        table.champions()
    } else {
        state.roster.clone()
    }
}

fn portrait(id: &str, x: usize, y: usize, size: usize) -> String {
    format!(r##"#{id}:color {{ x:{x}px; y:{y}px; width:{size}px; height:{size}px;
      color:#212734ff; rounding:Uniform{{rounding:6;}} ignore_event:true;
      #fallback:label {{ @"asset/base/style/main#bold_label"; width:{size}px; height:{size}px;
        align_x:Center; align_y:Center; size:16; text:"?"; color:#8190a2ff; ignore_event:true; }}
      #icon:image {{ anchor_x:0.5; pivot_x:0.5; anchor_y:0.5; pivot_y:0.5;
        width:{size}px; height:{size}px; ignore_event:true; }}
    }}"##)
}

fn eligible()->BTreeSet<String>{crate::mlbb::statistics_final_keys()}
fn patch(s:&State,t:&builds::Table)->Option<String>{s.patch.checked_sub(1).and_then(|i|t.patches().get(i).cloned())}
fn allowed(tier:usize)->BTreeSet<String>{eligible().into_iter().filter(|k|match tier{1=>!k.starts_with("mythical_"),2=>k.starts_with("mythical_"),_=>true}).collect()}
fn table(s: &State) -> builds::Table {
    let revision = s.balance_only.then(crate::patches::balance_revision);
    item_stats::build_table(revision.as_deref())
}
fn view(s:&State)->builds::View{let t=table(s);t.view(s.champ.as_deref().unwrap_or(""),s.lane.checked_sub(1),patch(s,&t).as_deref(),&allowed(s.tier))}
pub(crate) fn career_started(){if let Ok(mut s)=STATE.lock(){*s=State{patch:1,..Default::default()};}}
fn icon(id:&str,x:usize,y:usize,key:&str)->String{format!("#{id}:image {{ignore_event:true;x:{x}px;y:{y}px;width:24px;height:24px;source:\"asset/mlbb_items/aseprite_resources/ingame/item_icons_640X640\";rect_tag:\"{}\";}}",safe(key))}
// The authored Statistics tree is rebuilt during day simulation. A path cache
// must never stand in for the lifetime of that tree or keep an old tab active.
const VISIT:&str="main.top.right.statistics.mlbb_build_stats_visit";
fn close_visit(s: &mut State) {
    s.open = false;
    s.champ = None;
    s.page = 0;
    s.details = false;
    s.history_only = false;
    s.roster.clear();
    s.shown_champs.clear();
    reset_preview(s);
    s.message.clear();
    s.painted.clear();
    s.drawn_for = None;
}
fn reset_preview(s: &mut State) {
    s.preview = None;
    s.planner = false;
    s.library = None;
    s.library_index = 0;
    s.pending_import = false;
    s.selected_slot = 0;
    s.item_page = 0;
    s.undo_slots = None;
}
fn current_plan(rows: &[ChampionRow], champion: &str, role: Role) -> ChampionRow {
    rows.iter().find(|row| row.champion.as_deref() == Some(champion) && row.role == role)
        .cloned().unwrap_or_else(|| {
            let mut row = rows.iter().find(|row| row.champion.as_deref() == Some(champion) && row.role == Role::Any)
                .cloned().unwrap_or_default();
            row.champion = Some(champion.to_owned());
            row.role = role;
            row.slots.resize(6, None);
            row
        })
}
fn planned_order(row: &ChampionRow) -> Vec<Option<String>> {
    let mut slots = row.slots.clone();
    crate::role_gear::apply_priorities(&mut slots, row.gear, row.champion.as_deref(), row.role);
    if let Some(champion) = row.champion.as_deref() {
        slots = crate::role_gear::roaming_first_row(champion, row.role, row.gear, &slots);
    }
    slots.resize(6, None);
    slots
}
fn build_error(row: &ChampionRow, catalog: &std::collections::BTreeMap<String, item_stats::ItemInfo>) -> Option<&'static str> {
    let mut seen = BTreeSet::new();
    let known = eligible();
    for key in planned_order(row).iter().flatten() {
        if !known.contains(key) && !catalog.contains_key(key) {
            return Some("The draft contains an unavailable item.");
        }
        if !seen.insert(builds::family(key).to_owned()) {
            return Some("The draft repeats an item family.");
        }
    }
    None
}

const STATS_PANELS: [&str; 4] = ["champion", "athlete", "team", "item_stats"];
const NATIVE_FILTERS: [&str; 4] = ["position", "patch", "year_filter", "league_filter"];
const ITEM_FILTERS: [&str; 9] = [
    "item_lane", "item_tier", "item_category", "item_patch", "item_lane_list",
    "item_tier_list", "item_category_list", "item_patch_list", "item_category_catch",
];

fn suppress_other_stats_ui(ctx: &mut StableClient<'_>) {
    for id in STATS_PANELS {
        ctx.ui_set_visible(&format!("{ROOT}.data.{id}"), false);
    }
    for id in NATIVE_FILTERS.into_iter().chain(ITEM_FILTERS) {
        ctx.ui_set_visible(&format!("{ROOT}.{id}"), false);
    }
}

fn restore_native(ctx:&mut StableClient<'_>,tab:&str){
 let panel=if tab=="item"{"item_stats"}else{tab};
 ctx.ui_set_visible(PANEL,false);
 ctx.ui_set_visible(SEARCH,false);
 ctx.ui_set_visible(NAME,false);
 ctx.ui_set_visible(ITEM_SEARCH,false);
 for id in STATS_PANELS{ctx.ui_set_visible(&format!("{ROOT}.data.{id}"),id==panel);}
 for id in NATIVE_FILTERS{ctx.ui_set_visible(&format!("{ROOT}.{id}"),tab!="item");}
 for id in ["champion","athlete","team","item","build_stats"]{
  let p=format!("{ROOT}.tabs.{id}");let lit=id==tab;
  ctx.ui_set_properties(&p,&crate::strategy_ui::tab_style("image","label",lit));
  ctx.ui_set_properties(&p,&crate::strategy_ui::tab_style("selected_image","selected_label",lit));
 }
}
fn maintain_screen(ctx:&mut StableClient<'_>)->bool{
 if !ctx.ui_exists(ROOT){
  if let Ok(mut s)=STATE.lock(){close_visit(&mut s);}
  EVENTS.forget();return false
 }
 if ctx.ui_visible(ROOT)!=Some(true){
  let active=if let Ok(mut s)=STATE.lock(){let active=s.open;close_visit(&mut s);active}else{false};
  if active{restore_native(ctx,"champion");}
  // Re-arm on the next visit even if the engine kept the root node around.
  ctx.ui_remove_node(VISIT);return false
 }
 if !ctx.ui_exists(VISIT){
  // Also catches a rebuild entirely between two frames: the root path can
  // already exist again, but our unowned marker cannot survive its removal.
  if let Ok(mut s)=STATE.lock(){close_visit(&mut s);}
  EVENTS.forget();
  if !ctx.ui_spawn_source(ROOT,"mlbb_build_stats_visit:empty {ignore_event:true;width:0px;height:0px;}"){return false}
 }
 for id in ["build_stats","champion","athlete","team","item"]{EVENTS.register(ctx,&format!("{ROOT}.tabs.{id}"));}
 true
}
fn event(ctx:&mut StableClient<'_>){
 let Some(e)=ctx.ui_current_event()else{return};if !matches!(e.kind,Some(UiEventKindV1::Click)|None){return}
 if !e.path.starts_with(&format!("{ROOT}."))||!maintain_screen(ctx){return}
 let Some(id)=e.path.rsplit('.').next()else{return};let Ok(mut s)=STATE.lock()else{return};
    if e.path == format!("{ROOT}.tabs.build_stats") {
        item_stats::ui::leave_for_build_stats(ctx);
        suppress_other_stats_ui(ctx);
        s.open = true;
        s.roster_refresh = true;
        s.painted.clear();
        return;
    }
    if ["champion", "athlete", "team", "item"].contains(&id) && e.path == format!("{ROOT}.tabs.{id}") {
        let active = s.open;
        s.open = false;
        reset_preview(&mut s);
        if active {
            if id == "item" { item_stats::ui::open_from_build_stats(ctx); }
            restore_native(ctx, id);
        }
        return;
    }
    if !s.open || !e.path.starts_with(&format!("{PANEL}.")) { return; }

    if let Some(index) = id.strip_prefix("details_") {
        s.details = index == "1";
        s.page = 0;
    } else if let Some(index) = id.strip_prefix("patch_").and_then(|index| index.parse::<usize>().ok()) {
        s.patch = index;
        s.page = 0;
        reset_preview(&mut s);
    } else if let Some(index) = id.strip_prefix("tier_").and_then(|index| index.parse::<usize>().ok()) {
        s.tier = index.min(2);
        s.page = 0;
        reset_preview(&mut s);
    } else if let Some(index) = id.strip_prefix("champ_").and_then(|i| i.parse::<usize>().ok()) {
        s.champ = s.shown_champs.get(index).cloned();
        s.page = 0;
        reset_preview(&mut s);
    } else if id == "history_mode" {
        s.history_only = !s.history_only;
        s.champ = None;
        s.page = 0;
        reset_preview(&mut s);
    } else if let Some(index) = id.strip_prefix("role_").and_then(|index| index.parse::<usize>().ok()) {
        s.lane = index.min(5);
        s.page = 0;
        reset_preview(&mut s);
    } else if let Some(index) = id.strip_prefix("page_").and_then(|index| index.parse::<usize>().ok()) {
        s.page = index.min(1000);
    } else if id == "balance" {
        s.balance_only = !s.balance_only;
        s.patch = 0;
        s.page = 0;
        reset_preview(&mut s);
    } else if id == "back" {
        s.champ = None;
        s.page = 0;
        reset_preview(&mut s);
    } else if id == "cancel" {
        reset_preview(&mut s);
    } else if let Some(champion) = s.champ.clone() {
        planner_event(ctx, &mut s, id, &champion);
    }
    s.painted.clear();
}

fn planner_event(ctx: &mut StableClient<'_>, state: &mut State, id: &str, champion: &str) {
    use crate::build_library::{Library, Plan};
    if state.history_only || !state.roster.contains(champion) {
        return;
    }
    let role = Role::ALL[state.lane];
    let policy = (crate::first_slot_rules::current().code(), crate::item_tiers::rule().code());
    if let Some(slot) = id.strip_prefix("select_slot_").and_then(|s| s.parse::<usize>().ok()) {
        state.selected_slot = slot.min(5);
        return;
    }
    if let Some(key) = id.strip_prefix("pick_item_") {
        if let Some((row, _)) = state.preview.as_mut().filter(|_| eligible().contains(key)) {
            state.undo_slots = Some(row.slots.clone());
            row.slots.resize(6, None);
            row.slots[state.selected_slot] = Some(key.to_owned());
        }
        return;
    }
    match id {
        "preview" | "planner" => {
            let rows = build_config::load_champion_rows();
            let mut row = current_plan(&rows, champion, role);
            if id == "preview" {
                let filled = builds::fill_open_slots(&view(state), &row.slots);
                if filled.iter().zip(row.slots.iter().chain(std::iter::repeat(&None)))
                    .all(|(suggested, current)| suggested == current) {
                    state.message = "No open slot has an eligible recommendation with at least 10 games.".into();
                    return;
                }
                row.slots = filled;
                let ordered = planned_order(&row);
                if current_plan(&rows, champion, role).slots.iter().flatten().any(|pin| !ordered.contains(&Some(pin.clone()))) {
                    state.message = "First-slot rules would displace a pin. Open the planner to choose which slot to clear.".into();
                    return;
                }
            }
            state.planner = true;
            state.library = Library::load().ok();
            state.policy = policy;
            state.undo_slots = None;
            state.preview = Some((row, rows));
            state.message.clear();
        }
        "favorite_item" => {
            let Some(key) = state.preview.as_ref().and_then(|(row, _)| row.slots.get(state.selected_slot))
                .and_then(Option::as_deref) else { return; };
            state.message = Library::load().and_then(|mut library| {
                let new = library.toggle_favorite(key);
                library.save()?;
                Ok(new)
            }).map_or_else(str::to_owned, |added| if added {"Added to favorite equipment.".into()}
                else {"Removed from favorite equipment.".into()});
            state.library = Library::load().ok();
        }
        "item_previous" => state.item_page = state.item_page.saturating_sub(1),
        "item_next" => state.item_page = state.item_page.saturating_add(1),
        "clear_slot" => {
            if let Some((row, _)) = &mut state.preview {
                state.undo_slots = Some(row.slots.clone());
                row.slots.resize(6, None);
                row.slots[state.selected_slot] = None;
            }
        }
        "undo_item" => {
            if let (Some((row, _)), Some(slots)) = (&mut state.preview, state.undo_slots.take()) {
                row.slots = slots;
            }
        }
        "library_next" => {
            state.library_index = state.library_index.saturating_add(1);
            state.pending_import = false;
        }
        "library_review" => {
            let found = Library::load().ok().and_then(|library| {
                let count = library.matching(champion, role).count();
                library.matching(champion, role).nth(state.library_index % count.max(1)).cloned()
            });
            if let Some(plan) = found {
                state.preview = Some((plan.row(), build_config::load_champion_rows()));
                state.policy = policy;
                state.undo_slots = None;
                state.message = format!("Reviewing {}.", plan.name);
            }
        }
        "library_save" => {
            let Some((row, _)) = &state.preview else { return; };
            let name = ctx.ui_text_edit_text(NAME).unwrap_or_default();
            let result = Library::load().and_then(|mut library| {
                library.append(Plan::new(&name, row)?)?;
                library.save()
            });
            if let Ok(()) = result {
                state.library = Library::load().ok();
            }
            state.message = result.map_or_else(str::to_owned, |_| "Saved as a named alternative.".into());
        }
        "library_export" => {
            state.message = Library::load().and_then(|library| library.export()).map_or_else(
                str::to_owned, |_| "Exported build-library-export.json in the mod folder.".into());
        }
        "library_import" => {
            let result = Library::load().and_then(|mut library| {
                let (added, skipped) = library.merge(Library::import()?);
                if state.pending_import {
                    library.save()?;
                }
                Ok((added, skipped))
            });
            state.message = match result {
                Ok((added, skipped)) if state.pending_import => {
                    state.pending_import = false;
                    state.library = Library::load().ok();
                    format!("Imported {added} plans. Skipped {skipped} duplicates or entries over the limit.")
                }
                Ok((added, skipped)) => {
                    state.pending_import = true;
                    format!("Import review · Add {added} · Skip {skipped} · Select Confirm Import to save.")
                }
                Err(error) => {
                    state.pending_import = false;
                    format!("{error} Place build-library-import.json in the mod folder.")
                }
            };
        }
        "apply" if !super::options_row::locked_now(ctx) => {
            let Some((row, baseline)) = &state.preview else { return; };
            let mut current = build_config::load_champion_rows();
            if &current != baseline || policy != state.policy {
                state.message = "The active plans changed. Reopen the preview to compare again.".into();
            } else if !row.is_complete() {
                state.message = "The draft needs an item or role setting before applying.".into();
            } else if let Some(error) = build_error(row, &item_stats::catalog()) {
                state.message = error.into();
            } else {
                let mut new = row.clone();
                new.slots = planned_order(row);
                if let Some(index) = current.iter().position(|r| r.champion == row.champion && r.role == row.role) {
                    current[index] = new;
                } else {
                    current.push(new);
                }
                if build_config::save_champion_rows(&current) {
                    reset_preview(state);
                    state.message = "Applied to this champion and role.".into();
                } else {
                    state.message = "Could not write the active planned builds.".into();
                }
            }
        }
        _ => {}
    }
}
pub(crate) fn sync(ctx:&mut StableClient<'_>){
 if !maintain_screen(ctx){return}
 let Ok(mut s)=STATE.lock()else{return};
 if !s.open {
    if ctx.ui_visible(PANEL) == Some(true) { ctx.ui_set_visible(PANEL, false); }
    return;
 }
 // TFM2 reasserts its selected tab's controls on every frame. Keep those
 // hidden even on the frames when there is no Build Stats table redraw.
 suppress_other_stats_ui(ctx);
 static FRAME:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0);
 let frame = FRAME.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
 if frame % 10 != 0 && !s.roster_refresh { return; }
 if s.roster_refresh || frame % 60 == 0 || s.roster.is_empty() {
    let current: BTreeSet<_> = super::active_champion_ids(ctx).into_iter().collect();
    if (!current.is_empty() || s.roster_refresh) && current != s.roster {
        s.roster = current;
        s.drawn_for = None;
    }
    s.roster_refresh = false;
    if !s.history_only && s.champ.as_ref().is_some_and(|champ| !s.roster.contains(champ)) {
        s.champ = None;
        s.page = 0;
        reset_preview(&mut s);
    }
 }
 if !ctx.ui_exists(PANEL) && !ctx.ui_spawn_source(ROOT, &shell_source()) {
    crate::patches::log("build_stats.shell", || "Build Stats could not mount its layout.".into());
    return;
 }
 let query = if s.planner {
    ctx.ui_text_edit_text(ITEM_SEARCH).unwrap_or_default()
 } else if s.champ.is_none() {
    ctx.ui_text_edit_text(SEARCH).unwrap_or_default().to_lowercase()
 } else {
    String::new()
 };
 let stamp = DrawInput {
    observations: item_stats::build_revision(),
    balance: crate::patches::balance_revision(),
    query,
    language: crate::patches::text::game_language(),
    locked: super::options_row::locked_now(ctx),
 };
 if !s.painted.is_empty() && s.drawn_for.as_ref() == Some(&stamp) && ctx.ui_exists(CONTENT) {
    if ctx.ui_visible(PANEL) == Some(false) { ctx.ui_set_visible(PANEL, true); }
    return;
 }
 for id in ["champion","athlete","team","item","build_stats"]{let lit=id=="build_stats";let p=format!("{ROOT}.tabs.{id}");ctx.ui_set_properties(&p,&crate::strategy_ui::tab_style("image","label",lit));ctx.ui_set_properties(&p,&crate::strategy_ui::tab_style("selected_image","selected_label",lit));}
 let revision = s.balance_only.then_some(stamp.balance.as_str());
 let table = item_stats::build_table(revision);
 let chosen_patch = patch(&s,&table);
 let allowed=allowed(s.tier);
 ctx.ui_set_visible(SEARCH, s.champ.is_none());
 ctx.ui_set_visible(NAME, s.planner);
 ctx.ui_set_visible(ITEM_SEARCH, s.planner);
 let mut body=String::new();let mut buttons=Vec::new();let mut portraits=Vec::new();
 macro_rules! b{($id:expr,$x:expr,$y:expr,$w:expr,$text:expr)=>{{let id=$id.to_string();body+=&button(&id,$x,$y,$w,$text);buttons.push(id);}}}
 if s.planner {
  draw_plan(ctx, &mut s, &table, chosen_patch.as_deref(), &stamp.query, &mut body, &mut buttons);
 } else if let Some(c)=s.champ.as_deref(){
  body+=&label("name",84,20,600,&cname(ctx,c),24);body+=&portrait("portrait",20,16,48);portraits.push(("portrait".to_string(),c.to_string(),48.));b!("back",1450,16,120,"Back");
  for(i,r)in Role::ALL.iter().enumerate() {
      let v=table.view(c,i.checked_sub(1),chosen_patch.as_deref(),&allowed);
      let id = format!("role_{i}");
      let title = format!("{} · {} · {} games",r.label(),rate(v.hero),v.hero.games);
      body += &selection_button(&id,20+i*230,72,220,&title,i==s.lane);
      buttons.push(id);
  }
  let v=table.view(c,s.lane.checked_sub(1),chosen_patch.as_deref(),&allowed);let rec=builds::recommended(&v);let cat=item_stats::catalog();
  body+=&label("summary",20,110,1000,&format!("{} win rate · {} games",rate(v.hero),v.hero.games),15);
  b!(format!("details_{}",u8::from(!s.details)),1410,110,160,if s.details{"Compact View"}else{"Item Details"});
  if v.hero.games > 0 {
  body+="#recommended:color {x:20px;y:150px;width:1550px;height:104px;color:#1b1d29ff;rounding:Uniform{rounding:8;}}";
  for slot in 0..6{
   let x=30+slot*258;
   body+=&label(&format!("besthead{slot}"),x,154,240,&format!("Slot {} recommendation",slot+1),13);
   if let Some(key)=&rec[slot]{
    let title=cat.get(key).map(|i|i.name.as_str()).unwrap_or(key);
    body+=&icon(&format!("besticon{slot}"),x,188,key);
    body+=&label(&format!("bestname{slot}"),x+30,184,212,&short_name(title),12);
    if let Some(row)=v.slots[slot].iter().find(|r|&r.key==key){body+=&rate_label(&format!("bestrate{slot}"),x,216,240,&format!("{} WR · {} games",rate(row.count),row.count.games),13,row.count);}
   }else{body+=&label(&format!("bestempty{slot}"),x,186,240,"Needs 10 games on one item",12);}
  }
  if s.details{
   body+=&label("detail_title",24,266,1500,"Item details · Adjusted rates rank suggestions; fewer than 10 games cannot qualify",14);
   for(id,x,w,t)in [("ds",24,60,"Slot"),("di",100,600,"Item"),("dw",760,140,"Win Rate"),("dp",920,140,"Pick Rate"),("dg",1080,130,"Games"),("da",1230,300,"Adjusted WR")]{body+=&label(id,x,302,w,t,13);}
   let rows:Vec<_>=v.slots.iter().enumerate().flat_map(|(slot,rows)|rows.iter().map(move|r|(slot,r))).collect();
   for(i,(slot,r))in rows.iter().skip(s.page*10).take(10).enumerate(){let y=338+i*31;let title=cat.get(&r.key).map(|i|i.name.as_str()).unwrap_or(&r.key);
    body+=&label(&format!("ds{i}"),24,y,60,&(slot+1).to_string(),13);body+=&icon(&format!("di{i}"),100,y,&r.key);body+=&label(&format!("dn{i}"),134,y,606,title,13);
    body+=&rate_label(&format!("dw{i}"),760,y,140,&rate(r.count),13,r.count);body+=&label(&format!("dp{i}"),920,y,140,&format!("{:.1}%",r.picks),13);body+=&label(&format!("dg{i}"),1080,y,130,&r.count.games.to_string(),13);body+=&label(&format!("da{i}"),1230,y,250,&format!("{:.1}%",r.count.adjusted()),13);
   }
  }else{
   for slot in 0..6{
    let x=20+slot*260;body+=&format!("#slot{slot}:color {{x:{x}px;y:270px;width:250px;height:388px;color:#0f1016ff;rounding:Uniform{{rounding:8;}}}}");
    body+=&label(&format!("sh{slot}"),x+10,274,230,&format!("Slot {} · {} games reached",slot+1,v.denominators[slot]),13);
    for(id,offset,w,t)in [("wr",10,82,"Win Rate"),("pr",98,82,"Pick Rate"),("g",184,60,"Games")]{body+=&label(&format!("{id}{slot}"),x+offset,304,w,t,11);}
    for(i,r)in v.slots[slot].iter().skip(s.page*5).take(5).enumerate(){let y=338+i*62;let title=cat.get(&r.key).map(|i|i.name.as_str()).unwrap_or(&r.key);
     body+=&icon(&format!("icon{slot}_{i}"),x+10,y,&r.key);body+=&label(&format!("n{slot}_{i}"),x+40,y,202,&short_name(title),12);
     body+=&rate_label(&format!("wr{slot}_{i}"),x+10,y+26,82,&rate(r.count),12,r.count);body+=&label(&format!("pr{slot}_{i}"),x+98,y+26,82,&format!("{:.1}%",r.picks),12);body+=&label(&format!("g{slot}_{i}"),x+184,y+26,60,&r.count.games.to_string(),12);
    }
   }
  }
  } else {
    let empty = if s.balance_only {
        "Play a match using the current item balance to begin this history."
    } else if table.champions().is_empty() {
        "Play a match to begin collecting final equipped item slots."
    } else {
        "Try a different role or game patch."
    };
    body += "#empty_card:color {x:20px;y:155px;width:1550px;height:180px;color:#1a2029ff;rounding:Uniform{rounding:10;}}";
    body += &label("empty_title", 46, 177, 1400, "No matches in this view", 21);
    body += &label("empty_help", 46, 220, 1400, empty, 16);
    if s.details {
        body += &label("detail_title",46,267,1420,"Item details show games, raw WR and Adjusted WR once matches are recorded.",14);
    }
  }
  if v.hero.games > 0 {
      if s.page > 0 { b!(format!("page_{}",s.page-1),20,680,110,"Previous"); }
      let another = if s.details { v.slots.iter().map(Vec::len).sum::<usize>() > (s.page+1)*10 }
          else { v.slots.iter().any(|rows| rows.len() > (s.page+1)*5) };
      if another { b!(format!("page_{}",s.page+1),140,680,110,"Next"); }
  }
  if s.history_only {
      body += &label("historical_only",1120,682,440,"Historical results · Read only",14);
  } else {
      b!("planner",1115,680,194,"Build Planner");
      if rec.iter().any(Option::is_some) {
          b!("preview",1320,680,250,"Review Suggestions");
      } else {
          body += &label("suggestion_status",1320,682,250,"Needs 10 Item Games",13);
      }
  }
  body+=&label("sample",270,682,840,
      if s.details {"Adjusted WR approaches raw WR as the sample grows to 60 games."}
      else {"Pick rate uses games that reached the slot. Slots show final equipped positions."},12);
  if v.hero.games > 0 && v.slots.iter().all(Vec::is_empty) {
      body += &label("tier_empty", 24, 724, 1500, "No qualifying equipment for this item tier. Try All Item Tiers.", 15);
  }
 }else{
  body+=&label("title",20,16,1000,"Build Stats",24);body+=&label("intro",20,52,1500,"Choose a champion to inspect item win rates and popularity in each build slot.",15);
  let champs = champion_rows(&s, &table);
  b!("history_mode",426,82,230,if s.history_only {"Match History"} else {"Active Champions"});
  let counts = table.hero_counts(chosen_patch.as_deref());
  let mut champs: Vec<_> = champs.into_iter().filter_map(|c| {
      let name = cname(ctx, &c);
      let matched = stamp.query.split_whitespace().all(|word| name.to_lowercase().contains(word) || c.contains(word));
      let count = counts.get(&c).copied().unwrap_or_default();
      matched.then_some((c, name, count))
  }).collect();
  champs.sort_by(|(_, name_a, a), (_, name_b, b)| b.adjusted().total_cmp(&a.adjusted())
      .then(b.games.cmp(&a.games)).then(name_a.cmp(name_b)));
  s.page = s.page.min(champs.len().saturating_sub(1) / 12);
  for(id,x,w,title)in [("hc",80,460,"Champion"),("hg",580,180,"Games"),("hw",800,200,"Win rate"),("ha",1050,300,"Adjusted win rate")]{body+=&label(id,x,126,w,title,15);}
  s.shown_champs.clear();
  for (i, (champion, name, count)) in champs.iter().skip(s.page*12).take(12).enumerate() {
      let y = 165+i*50;
      s.shown_champs.push(champion.clone());
      b!(format!("champ_{i}"),72,y,460,name);
      body += &label(&format!("g{i}"),580,y+4,180,&count.games.to_string(),15);
      body += &rate_label(&format!("w{i}"),800,y+4,200,&rate(*count),15,*count);
      let adjusted = if count.games>=10 { format!("{:.1}%", count.adjusted()) } else { "Not enough games".into() };
      body += &label(&format!("a{i}"),1050,y+4,200,&adjusted,15);
      let id = format!("portrait{i}");
      body += &portrait(&id,24,y+2,32);
      portraits.push((id,champion.clone(),32.));
  }
  if s.page > 0 { b!(format!("page_{}",s.page-1),20,800,130,"Previous"); }
  if (s.page+1)*12 < champs.len() { b!(format!("page_{}",s.page+1),160,800,130,"Next"); }
  if champs.is_empty() {
      let message = if !s.history_only && s.roster.is_empty() {
          "Waiting for this career's champion roster."
      } else if s.history_only && table.champions().is_empty() {
          "This balance has no saved champion history."
      } else {
          "No champions match your search."
      };
      body += &label("no_search",24,165,1460,message,16);
  }
  let history = if s.balance_only {"Current balance includes games tagged with the active item values."}
      else {"All balance history includes older matches without a recorded item balance."};
  body += &label("history",320,802,1230,history,13);
 }
 if !s.planner {
 let patch_count=table.patches().len();let next_patch=if s.patch>=patch_count{0}else{s.patch+1};
 b!("balance",700,16,190,if s.balance_only {"Current Balance"} else {"All Balances"});
 b!(format!("patch_{next_patch}"),900,16,300,&chosen_patch.as_ref().map(|p|format!("Patch {p}")).unwrap_or_else(||"All Patches".into()));
 b!(format!("tier_{}",(s.tier+1)%3),1210,16,220,["All Item Tiers","Finished Items","Mythical Items"][s.tier]);
 if s.champ.is_none() && table.champions().is_empty() {
     body+=&label("empty",28,844,1500,"No slot history for this balance yet. New matches will populate it.",14);
 }
 }
 body+=&label("message",24,888,1530,&s.message,13);
 let source = format!("content:empty {{width:1600px; height:916px;{body}}}");
 if s.painted != source || !ctx.ui_exists(CONTENT) {
    ctx.ui_remove_node(CONTENT);
    if ctx.ui_spawn_source(PANEL, &source) {
        for id in buttons {
            EVENTS.register(ctx, &format!("{CONTENT}.{id}"));
        }
        for (id, champion, size) in portraits {
            let icon = format!("{CONTENT}.{id}.icon");
            if ctx.ui_set_champion_icon(&icon, &champion, size, size, 2.) {
                ctx.ui_set_visible(&format!("{CONTENT}.{id}.fallback"), false);
            }
        }
        s.painted = source;
    } else {
        s.drawn_for = None;
        return;
    }
 }
 s.drawn_for = Some(stamp);
 ctx.ui_set_visible(PANEL, true);
}

fn planner_items(
    catalog: &std::collections::BTreeMap<String, item_stats::ItemInfo>,
    query: &str,
    favorites: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let query = query.to_lowercase();
    let mut entries: Vec<_> = eligible().into_iter().filter_map(|key| {
        let name = item_name(catalog, Some(&key));
        let lower = name.to_lowercase();
        query.split_whitespace().all(|word| lower.contains(word) || key.contains(word))
            .then_some((key, name))
    }).collect();
    entries.sort_by(|a, b| favorites.contains(&b.0).cmp(&favorites.contains(&a.0)).then(a.1.cmp(&b.1)));
    entries
}

fn item_name(
    catalog: &std::collections::BTreeMap<String, item_stats::ItemInfo>,
    key: Option<&String>,
) -> String {
    key.map(|key| catalog.get(key).map(|item| item.name.clone())
        .unwrap_or_else(|| key.replace('_', " "))).unwrap_or_else(|| "AI choice".into())
}

fn draw_plan(
    ctx: &StableClient<'_>,
    state: &mut State,
    table: &builds::Table,
    patch: Option<&str>,
    query: &str,
    body: &mut String,
    buttons: &mut Vec<String>,
) {
    let (Some(champion), Some((row, baseline))) = (&state.champ, &state.preview) else { return; };
    let cat = item_stats::catalog();
    let current = current_plan(baseline, champion, row.role);
    let ordered = planned_order(row);
    let apply_locked = super::options_row::locked_now(ctx);
    let invalid = build_error(row, &cat);
    let title = format!("Build Planner · {} · {}", cname(ctx, champion), row.role.label());
    body.push_str(&label("plan_title", 24, 18, 1250, &title, 24));
    body.push_str(&label("plan_subtitle", 24, 53, 1300, "Compare your active pins with the draft. Choose a slot to change it.", 15));

    let mut select = |id: &str, x, y, width, title: &str| {
        body.push_str(&button(id, x, y, width, title));
        buttons.push(id.into());
    };
    select("cancel", 1430, 20, 140, "Back to Stats");
    drop(select);

    body.push_str("#comparison:color {x:20px;y:101px;width:1110px;height:442px;color:#1a2029ff;rounding:Uniform{rounding:10;}}");
    body.push_str("#picks:color {x:1148px;y:101px;width:420px;height:581px;color:#1a2029ff;rounding:Uniform{rounding:10;}}");
    for (id, x, title) in [("slot_title",40,"Slot"),("before",172,"Active pin"),("after",474,"Draft pin"),("effective",773,"Effective target")] {
        body.push_str(&label(id, x, 112, 290, title, 15));
    }

    let mut route_total = Some((0usize, 0usize));
    for (slot, effective) in ordered.iter().enumerate() {
        let y = 151 + slot * 62;
        let selected = slot == state.selected_slot;
        let id = format!("select_slot_{slot}");
        body.push_str(&selection_button(&id, 36, y, 110, &format!("Slot {}", slot+1), selected));
        buttons.push(id);
        let old = current.slots.get(slot).and_then(Option::as_ref);
        let draft = row.slots.get(slot).and_then(Option::as_ref);
        let target = effective.as_ref().map(|key| crate::item_tiers::for_slot(key, slot).unwrap_or_else(|| key.clone()));
        for (index, x, key) in [(0, 169, old), (1, 472, draft), (2, 768, target.as_ref())] {
            if let Some(key) = key {
                body.push_str(&icon(&format!("plan_icon{slot}_{index}"), x, y+3, key));
            }
            body.push_str(&label(&format!("plan_name{slot}_{index}"), x+30, y, 264, &item_name(&cat, key), 14));
        }
        if let Some(key) = target {
            if let Some((low, high)) = super::item_spend::route_price_range(&key) {
                route_total = route_total.map(|(a,b)| (a.saturating_add(low), b.saturating_add(high)));
                let price = if low == high { format!("{low} G") } else { format!("{low}–{high} G") };
                body.push_str(&label(&format!("gold{slot}"), 798, y+24, 290, &price, 12));
            } else {
                route_total = None;
            }
        }
    }
    let rule = ["All Mythicals", "First Item Mythical", "No Mythicals"][crate::item_tiers::rule().code() as usize];
    let first = crate::first_slot_rules::current();
    let gear = crate::first_slot_rules::apply(row.gear, row.role);
    let rule_text = format!("{rule} · Jungle First {} · Roam Boots First {} · Jungle {} · Roam {}",
        if first.jungle_first {"on"} else {"off"}, if first.roam_first {"on"} else {"off"},
        gear.jungle.label(), gear.roam.label());
    body.push_str(&label("order", 30, 550, 1080, &rule_text, 14));
    let gold = match route_total {
        Some((0,0)) => "Choose a pin to estimate its route cost.".into(),
        Some((low, high)) if high == low => format!("Estimated equipment route cost: {low} G. AI choices excluded."),
        Some((low, high)) => format!("Estimated equipment route cost: {low}–{high} G. AI choices excluded."),
        None => "Some draft pins have no known route price.".into(),
    };
    body.push_str(&label("est", 30, 578, 1080, &gold, 14));
    if let Some(error) = invalid {
        body.push_str(&label("plan_problem", 30, 606, 1060, error, 15));
    } else if let Some(key) = row.slots.get(state.selected_slot).and_then(Option::as_ref) {
        let context = table.view(champion, state.lane.checked_sub(1), patch, &allowed(state.tier));
        if let Some(sample) = context.slots[state.selected_slot].iter().find(|sample| &sample.key == key) {
            body.push_str(&label("plan_sample", 30, 606, 1060,
                &format!("Selected slot · {} win rate · {} games · {:.1}% pick rate",
                    rate(sample.count), sample.count.games, sample.picks), 14));
        }
    }

    let empty_favorites = BTreeSet::new();
    let favorites = state.library.as_ref().map(|library| library.favorites()).unwrap_or(&empty_favorites);
    let choices = planner_items(&cat, query, favorites);
    state.item_page = state.item_page.min(choices.len().saturating_sub(1) / 9);
    body.push_str(&label("pick_title",1166,156,365,&format!("Equipment for slot {}",state.selected_slot+1),15));
    if choices.is_empty() {
        body.push_str(&label("no_items",1170,198,350,"No equipment matches your search.",14));
    }
    for (index, (key, name)) in choices.iter().enumerate().skip(state.item_page*9).take(9) {
        let y = 191 + (index%9)*42;
        let id = format!("pick_item_{key}");
        let title = if favorites.contains(key) { format!("★  {name}") } else { name.clone() };
        body.push_str(&button(&id, 1206, y, 336, &title));
        buttons.push(id);
        body.push_str(&icon(&format!("pick_icon{index}"), 1173, y+4, key));
    }

    let (name, count) = state.library.as_ref().map(|library| {
        let count = library.matching(champion, row.role).count();
        let name = library.matching(champion, row.role)
            .nth(state.library_index % count.max(1)).map(|plan| plan.name.as_str());
        (name, count)
    }).unwrap_or((None, 0));
    let status = if let Some(name) = name { format!("{} of {count} · {name}", state.library_index % count + 1) }
        else { "No named alternatives for this champion and role.".into() };
    body.push_str(&label("library_title", 36, 686, 620, "Plan library", 18));
    body.push_str(&label("library", 36, 758, 530, &status, 15));

    let mut select = |id: &str, x, y, width, title: &str| {
        body.push_str(&button(id, x, y, width, title));
        buttons.push(id.into());
    };
    select("clear_slot", 1168, 610, 163, "Use AI Choice");
    if let Some(Some(key)) = row.slots.get(state.selected_slot) {
        select("favorite_item", 1168, 648, 193, if favorites.contains(key) {"Remove Favorite"} else {"Favorite Item"});
    }
    if state.undo_slots.is_some() { select("undo_item", 1340, 610, 200, "Undo Slot Edit"); }
    if state.item_page > 0 { select("item_previous", 1168, 570, 163, "Previous Items"); }
    if (state.item_page+1)*9 < choices.len() { select("item_next", 1340, 570, 200, "Next Items"); }
    select("library_save", 398, 721, 185, "Save Alternative");
    if count > 0 { select("library_next", 588, 753, 122, "Next Plan"); select("library_review", 720, 753, 136, "Review Plan"); }
    select("library_export", 36, 803, 169, "Export Library");
    select("library_import", 215, 803, 172, if state.pending_import {"Confirm Import"} else {"Review Import"});
    if !apply_locked && invalid.is_none() && row.is_complete() {
        select("apply", 1300, 803, 256, "Apply to Active Build");
    }
    drop(select);
    if apply_locked {
        body.push_str(&label("locked", 1110, 805, 430, "Apply from the management screen.", 14));
    }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]
 fn unavailable_champions_with_matches_are_kept_in_explicit_history_only() {
     let captured = crate::item_stats::sim::CapturedPlayer {
         champion: "cf_alistar".into(), lane: Some(0), items: vec!["boots".into()], won: true,
     };
     let mut table = builds::Table::default();
     table.fold("2026.0.0", &[captured]);
     let mut state = State { roster: BTreeSet::from(["mlbb_alpha".into()]), ..Default::default() };
     assert_eq!(champion_rows(&state, &table), BTreeSet::from(["mlbb_alpha".into()]));
     state.history_only = true;
     assert_eq!(champion_rows(&state, &table), BTreeSet::from(["cf_alistar".into()]));
     assert_eq!(table.hero_counts(None)["cf_alistar"].games, 1);
 }
 #[test]
 fn planner_inputs_share_the_persistent_shell_and_actions_keep_alignment() {
     let shell = shell_source();
     for id in ["champion_search", "plan_name", "equipment_search"] {
         assert!(shell.contains(&format!("#{id}:text_edit")));
     }
     let action = button("apply", 0, 0, 200, "Apply");
     assert!(action.contains("align_x:Center") && action.contains("align_y:Center"));
     let row = button("champ_0", 0, 0, 200, "Angela");
     assert!(row.contains("align_x:Left") && row.contains("align_y:Center"));
     let role = selection_button("role_0", 0, 0, 200, "Any", true);
     assert!(role.contains("color_selectable") && role.contains("align_y: Center"));
 }
 #[test]fn native_text_and_atlas_source_fix_the_rc16_font_and_sheet_errors(){
  let text=label("a",10,10,200,"a\nb\\n",14);
  assert!(text.contains("asset/base/style/main#label"));assert!(!text.contains("\\n"));
  let image=icon("i",10,10,"tough_boots");assert!(image.contains("item_icons_640X640\""));assert!(!image.contains("#sheet"));assert!(image.contains("rect_tag:"));
  assert_eq!(image.chars().filter(|c|*c=='{').count(),image.chars().filter(|c|*c=='}').count());
 }
 #[test]fn build_stats_is_a_fifth_vanilla_style_tab_inside_the_existing_statistics_screen(){
  let layout=include_str!("../../ui/layout/statistics.ui");assert!(layout.contains("#build_stats:color_selectable"));let tabs=layout.split("#position:dropdown").next().unwrap();assert_eq!(tabs.matches("width: 186px;").count()+tabs.matches("width:186px;").count(),5);
  assert_eq!(layout.chars().filter(|c|*c=='{').count(),layout.chars().filter(|c|*c=='}').count());assert!(!layout.contains("MLBB Item Tools"));
 }
}

#[cfg(test)]mod lifecycle_tests{
 use super::*;
 use std::{collections::{BTreeMap,BTreeSet},ffi::c_void,mem::size_of};
 #[derive(Default)]struct Host{nodes:BTreeSet<String>,visible:BTreeMap<String,bool>,callbacks:BTreeMap<String,(UiClickHandlerFn,*mut c_void)>,event:String,sources:Vec<String>}
 unsafe fn host<'a>(p:*const c_void)->&'a mut Host{&mut*(p as *mut Host)}
 unsafe fn text(p:*const u8,n:usize)->String{String::from_utf8_lossy(std::slice::from_raw_parts(p,n)).into_owned()}
 unsafe extern "C" fn exists(s:*const c_void,p:*const u8,n:usize)->bool{host(s).nodes.contains(&text(p,n))}
 unsafe extern "C" fn get_visible(s:*const c_void,p:*const u8,n:usize,out:*mut bool)->bool{*out=host(s).visible.get(&text(p,n)).copied().unwrap_or(true);true}
 unsafe extern "C" fn visible(s:*mut c_void,p:*const u8,n:usize,v:bool)->bool{host(s).visible.insert(text(p,n),v);true}
 unsafe extern "C" fn spawn(s:*mut c_void,p:*const u8,n:usize,src:*const u8,len:usize)->bool{
  let h=host(s);let source=text(src,len);let name=source.split(':').next().unwrap().trim();h.nodes.insert(format!("{}.{}",text(p,n),name));h.sources.push(source);true
 }
 unsafe extern "C" fn remove(s:*mut c_void,p:*const u8,n:usize)->bool{let path=text(p,n);host(s).nodes.retain(|k|k!=&path&&!k.starts_with(&format!("{path}.")));true}
 unsafe extern "C" fn register(s:*mut c_void,p:*const u8,n:usize,cb:Option<UiClickHandlerFn>,u:*mut c_void)->bool{if let Some(cb)=cb{host(s).callbacks.insert(text(p,n),(cb,u));}true}
 unsafe extern "C" fn props(_s:*mut c_void,_p:*const u8,_n:usize,_src:*const u8,_len:usize)->bool{true}
 unsafe extern "C" fn current(s:*const c_void,k:*mut u32,p:*mut u8,cap:usize,n:*mut usize,_payload:*mut u8,_pcap:usize,pn:*mut usize)->bool{
  let h=host(s);*k=UiEventKindV1::Click.code();*n=h.event.len();*pn=0;if !p.is_null()&&cap>=*n{std::ptr::copy_nonoverlapping(h.event.as_ptr(),p,*n);}true
 }
 fn run(h:&mut Host,f:impl FnOnce(&mut StableClient<'_>)){
  let mut ui:UiVtableV1=unsafe{std::mem::zeroed()};ui.size=size_of::<UiVtableV1>();ui.query_exists=Some(exists);ui.get_visible=Some(get_visible);ui.set_visible=Some(visible);ui.spawn_source=Some(spawn);ui.remove_node=Some(remove);ui.register_path_events=Some(register);ui.set_properties=Some(props);ui.current_event=Some(current);
  let mut raw:ClientCtxV1=unsafe{std::mem::zeroed()};raw.size=size_of::<ClientCtxV1>();raw.ui=&ui;raw.state=(h as *mut Host).cast();let mut ctx=unsafe{StableClient::from_raw(&mut raw).unwrap()};f(&mut ctx);
 }
 fn click(h:&mut Host,path:&str){h.event=path.into();assert!(h.callbacks.contains_key(path),"lost callback: {path}");run(h,event);}
 fn paint(h:&mut Host){for _ in 0..10{run(h,sync);}}
 #[test]fn simulated_day_rebuild_rebinds_tabs_and_back_without_forcing_the_old_panel(){
  career_started();EVENTS.forget();let mut h=Host::default();h.nodes.insert(ROOT.into());paint(&mut h);
  let native_controls = ["year_filter", "league_filter", "position", "patch", "data.champion"]
      .map(|id| format!("{ROOT}.{id}"));
  for path in &native_controls {
      h.visible.insert(path.clone(), true);
  }
  click(&mut h,&format!("{ROOT}.tabs.build_stats"));
  for path in &native_controls {
      assert_eq!(h.visible.get(path), Some(&false), "{path} flashed on entry");
  }
  // Reproduce TFM2 re-showing its active tab's controls before our hook.
  // Check individual frames so a repaint on the tenth cannot mask flicker.
  for frame in 0..12 {
      for path in &native_controls {
          h.visible.insert(path.clone(), true);
      }
      run(&mut h, sync);
      for path in &native_controls {
          assert_eq!(h.visible.get(path), Some(&false), "{path} leaked on frame {frame}");
      }
  }
  {
      let mut state = STATE.lock().unwrap();
      state.roster.insert("mlbb_angela".into());
      state.champ = Some("mlbb_angela".into());
      state.painted.clear();
  }
  paint(&mut h);
  assert!(!h.callbacks.contains_key(&format!("{CONTENT}.preview")));
  let mounts = h.sources.iter().filter(|source| source.starts_with("mlbb_build_stats:color")).count();
  click(&mut h,&format!("{CONTENT}.planner"));
  paint(&mut h);
  assert_eq!(h.visible.get(ITEM_SEARCH), Some(&true));
  assert_eq!(h.visible.get(NAME), Some(&true));
  click(&mut h,&format!("{CONTENT}.cancel"));
  paint(&mut h);
  assert_eq!(h.visible.get(ITEM_SEARCH), Some(&false));
  assert_eq!(h.sources.iter().filter(|source| source.starts_with("mlbb_build_stats:color")).count(), mounts);
  STATE.lock().unwrap().page=3;
  // The engine removes its whole Statistics tree while simulating days.
  h.nodes.clear();h.callbacks.clear();paint(&mut h);assert!(!STATE.lock().unwrap().open);assert_eq!(STATE.lock().unwrap().page,0);
  h.nodes.insert(ROOT.into());paint(&mut h);assert_eq!(h.callbacks.len(),5);
  // Each native tab stays selected after another tick; its table is restored.
  for tab in ["champion","athlete","team","item"]{
   click(&mut h,&format!("{ROOT}.tabs.build_stats"));paint(&mut h);click(&mut h,&format!("{ROOT}.tabs.{tab}"));paint(&mut h);
   assert!(!STATE.lock().unwrap().open);assert_eq!(h.visible.get(PANEL),Some(&false));let table=if tab=="item"{"item_stats"}else{tab};assert_eq!(h.visible.get(&format!("{ROOT}.data.{table}")),Some(&true));
   for filter in NATIVE_FILTERS {
       assert_eq!(h.visible.get(&format!("{ROOT}.{filter}")), Some(&(tab != "item")));
   }
  }
  // Catch a full rebuild even if the root is already back at the next tick.
  click(&mut h,&format!("{ROOT}.tabs.build_stats"));h.nodes.clear();h.callbacks.clear();h.nodes.insert(ROOT.into());paint(&mut h);assert!(!STATE.lock().unwrap().open);assert_eq!(h.callbacks.len(),5);
  click(&mut h,&format!("{ROOT}.tabs.build_stats"));paint(&mut h);
  STATE.lock().unwrap().roster.insert("mlbb_angela".into());
  {
      let mut state = STATE.lock().unwrap();
      state.champ = Some("mlbb_angela".into());
      state.painted.clear();
  }
  paint(&mut h);click(&mut h,&format!("{CONTENT}.back"));paint(&mut h);assert!(STATE.lock().unwrap().champ.is_none());
  // The compact page and details toggle share the same safe navigation.
  {
      let mut state = STATE.lock().unwrap();
      state.champ = Some("mlbb_angela".into());
      state.painted.clear();
  }
  paint(&mut h);click(&mut h,&format!("{CONTENT}.details_1"));paint(&mut h);assert!(h.sources.last().unwrap().contains("Adjusted WR"));assert!(h.sources.last().unwrap().contains("Item details"));
  h.visible.insert(ROOT.into(),false);paint(&mut h);assert!(!STATE.lock().unwrap().open);assert!(!h.nodes.contains(VISIT));
  career_started();EVENTS.forget();
 }
}
