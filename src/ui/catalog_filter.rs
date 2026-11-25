//! Presentation-only stock filtering. Registry IDs and saved inventories stay intact.
//! In Game Info and the Builds popup, the catalog keeps native selection and
//! headers. The mod owns the scrollable body for an MLBB item while it is mounted.
use mod_api_stable::StableClient;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

fn stock_key(key: &str) -> bool {
    key == "ironsword"
        || crate::item_catalog::GAME_ITEM_NAMES
            .iter()
            .any(|(_, native)| *native == key)
}
fn stock_label(label: &str) -> bool {
    let label = label.trim();
    // Reserved marker added only to stock entries by our translation generator.
    if label.starts_with("[TFM2]") {
        return true;
    }
    label
        .strip_prefix("#asset/base/text/item?")
        .and_then(|s| s.strip_suffix(".name"))
        .is_some_and(stock_key)
}
trait CatalogUi {
    fn exists(&self, path: &str) -> bool;
    fn children(&self, path: &str) -> Vec<String>;
    fn text(&self, path: &str) -> Option<String>;
    #[cfg(test)]
    fn hide(&mut self, path: &str);
}
impl CatalogUi for StableClient<'_> {
    fn exists(&self, path: &str) -> bool {
        self.ui_exists(path)
    }
    fn children(&self, path: &str) -> Vec<String> {
        self.ui_child_names(path)
    }
    fn text(&self, path: &str) -> Option<String> {
        self.ui_text(path)
    }
    #[cfg(test)]
    fn hide(&mut self, path: &str) {
        self.ui_set_visible(path, false);
    }
}
fn stock_cards(ui: &impl CatalogUi, contents: &str) -> Vec<String> {
    // Headers and MLBB cards retain their existing order and visibility.
    ui.children(contents).into_iter().filter_map(|child| {
        let path = format!("{contents}.{child}");
        let is_stock = stock_key(&child)
            || ["name", "name_bg.name"].iter().any(|label| {
                ui.text(&format!("{path}.{label}"))
                    .is_some_and(|text| stock_label(&text))
            });
        is_stock.then_some(path)
    }).collect()
}

#[cfg(test)]
fn filter_cards(ui: &mut impl CatalogUi, contents: &str) {
    for path in stock_cards(ui, contents) {
        ui.hide(&path);
    }
}

// Game Info mounts item_info.ui under main.top.right/main.contents, while
// Builds mounts it inside item_info_popup.popup.content. Discover the page
// itself rather than assuming every catalog belongs to the popup.
fn discover(ui: &impl CatalogUi) -> Vec<String> {
    let mut level = vec![
        "main".to_owned(),
        "main.top.right".to_owned(),
        "main.contents".to_owned(),
    ];
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    let mut budget = 512;
    for _ in 0..7 {
        let mut next = Vec::new();
        for path in level {
            if !seen.insert(path.clone()) {
                continue;
            }
            let candidate = format!("{path}.data.item_list.contents");
            if ui.exists(&candidate) {
                found.push(candidate);
                // Do not traverse hundreds of item cards and their labels.
                continue;
            }
            budget -= 1;
            if budget == 0 {
                return found;
            }
            for child in ui.children(&path) {
                next.push(format!("{path}.{child}"));
            }
        }
        level = next;
    }
    found
}
pub(crate) const RECIPE_PANE: &str = "mlbb_recipe_detail";
const NATIVE_DETAIL: [&str; 3] = ["desc", "sub_items_section", "upper_items_section"];
const LINE_H: u32 = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Viewport {
    width: u32,
    top: u32,
    height: u32,
}

impl Viewport {
    fn read(ctx: &StableClient<'_>, detail: &str) -> Option<Self> {
        let (_, y, width, height) = ctx.ui_node_rect(detail)?;
        let header_bottom = ["name", "price", "tier", "gold", "icon"].into_iter()
            .filter_map(|id| ctx.ui_node_rect(&format!("{detail}.{id}")))
            .map(|(_, child_y, _, child_height)| (child_y + child_height - y).max(0.0))
            .fold(80.0f32, f32::max);
        let top = header_bottom.ceil() as u32 + 12;
        let width = (width.floor() as u32).saturating_sub(40);
        let height = (height.floor() as u32).saturating_sub(top + 16);
        (width >= 180 && height >= 96).then_some(Self { width, top, height })
    }

    fn dimensions(self) -> String {
        format!("x:20px; y:{}px; width:{}px; height:{}px;", self.top, self.width, self.height)
    }

    fn source(self) -> String {
        let dimensions = self.dimensions();
        format!(r##"{RECIPE_PANE}:scroll_view {{
          {dimensions} visible:false; focusable:true; speed:100; bar_width:5;
          bar: {{ source:"asset/base/sprite/white"; color:#53dcc8ff; }}
          back: {{ source:"asset/base/sprite/white"; color:#394452ff; }}
        }}"##)
    }
}

#[derive(PartialEq)]
struct Detail {
    key: String,
    width: u32,
    stats: Vec<String>,
    effect: String,
    rows: [Vec<(&'static str, Option<usize>)>; 2],
}

impl Detail {
    fn read(key: &str, width: u32) -> Option<Self> {
        let (previous, next) = crate::mlbb::catalog_recipe(key)?;
        let rows = [previous, next].map(|keys| {
            keys.into_iter().map(|key| (key, super::item_spend::step_price(key))).collect()
        });
        let effect = crate::patches::text::option_now(key)
            .or_else(|| crate::patches::text::base_option(key).map(str::to_owned))
            .unwrap_or_default().trim().to_owned();
        Some(Self {
            key: key.to_owned(), width, rows, effect,
            stats: super::match_builds::catalog_stat_lines(key),
        })
    }
}

#[derive(Default)]
struct RecipePane {
    selected: Option<Detail>,
    visible_before: Option<[Option<bool>; 3]>,
    viewport: Option<Viewport>,
    alternate: bool,
    appearance: Option<((u64, u64), String)>,
    card_count: Option<usize>,
    stock: Vec<String>,
}

// 0.7.3 reports the label's allotted content rectangle, not its rendered text
// height. Reserve whole wrapped lines using the text at this pane's font size.
fn effect_height(effect: &str, width: u32) -> u32 {
    if effect.is_empty() {
        return 0;
    }
    let columns = (width / 12).max(1);
    let mut lines = 1;
    let mut used = 0;
    let mut token = 0;
    let mut tag = false;
    let mut gap = 0;
    let flush = |token: &mut u32, gap: &mut u32, used: &mut u32, lines: &mut u32| {
        if *token > 0 {
            if *used != 0 && *used + *gap + *token > columns {
                *lines += 1;
                *used = 0;
            }
            let next = *used + if *used == 0 { 0 } else { *gap } + *token;
            *lines += next.saturating_sub(1) / columns;
            *used = (next - 1) % columns + 1;
            *token = 0;
            *gap = 0;
        }
    };
    for character in effect.chars() {
        match character {
            '<' => tag = true,
            '>' => tag = false,
            _ if tag => {}
            '\n' => {
                flush(&mut token, &mut gap, &mut used, &mut lines);
                used = 0;
                gap = 0;
                lines += 1;
            }
            ch if ch.is_whitespace() => {
                flush(&mut token, &mut gap, &mut used, &mut lines);
                gap += 1;
            }
            ch => token += if ch.is_ascii() { 1 } else { 2 },
        }
    }
    flush(&mut token, &mut gap, &mut used, &mut lines);
    lines * LINE_H + 8
}

fn recipe_source(id: &str, detail: &Detail) -> String {
    use std::fmt::Write;
    let width = detail.width.saturating_sub(16);
    let name_width = width.saturating_sub(165);
    let cost_x = width.saturating_sub(110);
    let icon_sheet = crate::strategy_ui::ICON_SHEET;
    let stat_height = detail.stats.len() as u32 * LINE_H;
    let effect_height = effect_height(&detail.effect, width);
    let effect_y = stat_height + if stat_height > 0 && effect_height > 0 { 12 } else { 0 };
    let mut y = effect_y + effect_height;
    let mut rows = String::new();
    if stat_height > 0 {
        let _ = write!(rows, r##"#stats:label {{
          @"asset/base/style/main#label"; width:{width}px; height:{stat_height}px;
          size:18; line_height:{LINE_H}; ignore_event:true;
        }}"##);
    }
    if effect_height > 0 {
        let _ = write!(rows, r##"#effect:label {{
          @"asset/base/style/main#label"; y:{effect_y}px; width:{width}px;
          height:{effect_height}px; size:18; line_height:{LINE_H}; ignore_event:true;
        }}"##);
    }
    for (index, (heading, recipes)) in ["sub_items", "upper_items"].into_iter().zip(&detail.rows).enumerate() {
        if recipes.is_empty() {
            continue;
        }
        y += 12;
        let _ = write!(rows, r##"
          #heading{index}:label {{
            @"asset/base/style/main#bold_label"; y:{y}px; width:{width}px; height:28px;
            size:17; align_y:Center; color:#aab5c5ff; ignore_event:true;
            text:"#asset/base/text/item?related.{heading}";
          }}"##);
        y += 34;
        for (row, (key, price)) in recipes.iter().enumerate() {
            let price = price.map(|amount| format!("+{amount} G")).unwrap_or_default();
            let _ = write!(rows, r##"
              #recipe{index}_{row}:color {{
                y:{y}px; width:{width}px; height:40px; color:#191f2aff;
                rounding:Uniform{{rounding:6;}} ignore_event:true;
                #icon:image {{ x:4px; y:2px; width:36px; height:36px; source:"{icon_sheet}"; rect_tag:"{key}"; ignore_event:true; }}
                #title:label {{
                  @"asset/base/style/main#label"; x:52px; width:{name_width}px;
                  height:40px; size:17; align_y:Center; fit_width:true; ignore_event:true;
                  text:"#asset/base/text/item?{key}.name";
                }}
                #price:label {{
                  @"asset/base/style/main#label"; x:{cost_x}px; width:96px; height:40px;
                  size:14; align_x:Right; align_y:Center; color:#d6b76dff; ignore_event:true; text:"{price}";
                }}
              }}"##);
            y += 46;
        }
    }
    y += 10;
    format!("{id}:empty {{ width:{width}px; height:{y}px; visible:false; {rows} }}")
}

fn site_visible(ctx: &StableClient<'_>, detail: &str) -> bool {
    if ctx.ui_visible(detail) == Some(false) {
        return false;
    }
    let mut child = detail;
    while let Some((parent, _)) = child.rsplit_once('.') {
        if ctx.ui_visible(parent) == Some(false) {
            return false;
        }
        child = parent;
    }
    true
}

fn selection(ctx: &StableClient<'_>, path: &str) -> Option<String> {
    let label = ctx.ui_text(path)?;
    if let Some(key) = label.strip_prefix("#asset/base/text/item?")
        .and_then(|text| text.strip_suffix(".name"))
    {
        return Some(key.to_owned());
    }
    crate::patches::text::texts().key_of_name(&label).map(str::to_owned)
}

impl RecipePane {
    fn restore(&mut self, ctx: &mut StableClient<'_>, detail: &str, pane: &str) {
        if self.viewport.take().is_some() && ctx.ui_exists(pane) {
            ctx.ui_remove_node(pane);
        }
        if let Some(visible) = self.visible_before.take() {
            for (node, visible) in NATIVE_DETAIL.into_iter().zip(visible) {
                if let Some(visible) = visible {
                    ctx.ui_set_visible(&format!("{detail}.{node}"), visible);
                }
            }
        }
        self.selected = None;
        self.appearance = None;
    }

    fn filter_visible_catalog(&mut self, ctx: &mut StableClient<'_>, contents: &str, rescan: bool) {
        if !site_visible(ctx, contents) {
            self.card_count = None;
            return;
        }
        let count = ctx.ui_child_count(contents);
        if rescan || self.card_count != count {
            self.stock = stock_cards(ctx, contents);
            self.card_count = count;
        }
        for path in &self.stock {
            if ctx.ui_visible(path) != Some(false) {
                ctx.ui_set_visible(path, false);
            }
        }
    }

    fn suppress_native(&self, ctx: &mut StableClient<'_>, detail: &str) -> bool {
        NATIVE_DETAIL.into_iter().all(|node| {
            let path = format!("{detail}.{node}");
            match ctx.ui_visible(&path) {
                Some(true) => ctx.ui_set_visible(&path, false),
                Some(false) => true,
                None => !ctx.ui_exists(&path) || ctx.ui_set_visible(&path, false),
            }
        })
    }

    fn draw(&mut self, ctx: &mut StableClient<'_>, detail_path: &str, pane: &str, view: Viewport, detail: Detail) -> bool {
        if self.viewport.is_none() {
            if !ctx.ui_spawn_source(detail_path, &view.source()) {
                return false;
            }
        } else if self.viewport != Some(view) && !ctx.ui_set_properties(pane, &view.dimensions()) {
            return false;
        }
        self.viewport = Some(view);
        if self.selected.as_ref() == Some(&detail) {
            return true;
        }

        // Prepare the complete new body before retiring the old one. The scroll
        // runner remains the same, so an idle refresh does not reset its offset.
        let id = if self.alternate { "body0" } else { "body1" };
        let path = format!("{pane}.{id}");
        if !ctx.ui_spawn_source(pane, &recipe_source(id, &detail)) {
            return false;
        }
        let stats_ok = detail.stats.is_empty() || ctx.ui_set_text(&format!("{path}.stats"), &detail.stats.join("\n"));
        let effect_ok = detail.effect.is_empty() || ctx.ui_set_text(&format!("{path}.effect"), &detail.effect);
        if !stats_ok || !effect_ok || !ctx.ui_set_visible(&path, true) {
            return false;
        }
        if self.selected.is_some() {
            let previous = if self.alternate { "body1" } else { "body0" };
            if !ctx.ui_remove_node(&format!("{pane}.{previous}")) {
                return false;
            }
        }
        self.selected = Some(detail);
        self.alternate = !self.alternate;
        true
    }

    fn sync(&mut self, ctx: &mut StableClient<'_>, contents: &str, refresh: bool) {
        let Some(data) = contents.strip_suffix(".item_list.contents") else { return };
        let detail = format!("{data}.item_detail");
        let pane = format!("{detail}.{RECIPE_PANE}");
        if ctx.ui_visible(&detail) != Some(true) || !site_visible(ctx, &detail) {
            if self.viewport.is_some() {
                self.restore(ctx, &detail, &pane);
            }
            return;
        }
        if self.viewport.is_some() && !ctx.ui_exists(&pane) {
            self.viewport = None;
            self.selected = None;
            self.visible_before = None;
        }

        let Some(key) = selection(ctx, &format!("{detail}.name")) else {
            self.restore(ctx, &detail, &pane);
            return;
        };
        let changed = self.selected.as_ref().is_none_or(|selected| selected.key != key);
        if changed && crate::mlbb::catalog_recipe(&key).is_none() {
            self.restore(ctx, &detail, &pane);
            return;
        }
        if changed || refresh {
            let Some(view) = Viewport::read(ctx, &detail) else {
                self.restore(ctx, &detail, &pane);
                return;
            };
            let appearance = (
                crate::patches::item_description_revision(),
                crate::patches::text::game_language(),
            );
            if changed || self.viewport != Some(view) || self.appearance.as_ref() != Some(&appearance) {
                let Some(model) = Detail::read(&key, view.width) else {
                    self.restore(ctx, &detail, &pane);
                    return;
                };
                if !self.draw(ctx, &detail, &pane, view, model) {
                    self.restore(ctx, &detail, &pane);
                    return;
                }
                self.appearance = Some(appearance);
            }
        }

        let already_owned = self.visible_before.is_some();
        if !already_owned {
            self.visible_before = Some(NATIVE_DETAIL.map(|node| ctx.ui_visible(&format!("{detail}.{node}"))));
        }
        // The game can re-show these sections every frame. Content updates
        // and stock-card scans are on independent, slower cadences.
        if !self.suppress_native(ctx, &detail)
            || (!already_owned && !ctx.ui_set_visible(&pane, true))
        {
            self.restore(ctx, &detail, &pane);
        }
    }
}

pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    static FRAME: AtomicU32 = AtomicU32::new(0);
    static CONTENTS: Mutex<Vec<(String, RecipePane)>> = Mutex::new(Vec::new());
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    let Ok(mut cached) = CONTENTS.lock() else { return };
    cached.retain(|(path, _)| ctx.ui_exists(path));
    if frame % 30 == 0 {
        for path in discover(ctx) {
            if !cached.iter().any(|(known, _)| known == &path) {
                crate::patches::log(&format!("catalog.site.{path}"), || format!("stock filter found {path}"));
                cached.push((path, RecipePane::default()));
            }
        }
    }
    for (path, recipe) in cached.iter_mut() {
        let refresh = frame % 6 == 0;
        if refresh {
            recipe.filter_visible_catalog(ctx, path, frame % 60 == 0);
        }
        recipe.sync(ctx, path, refresh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    #[derive(Default)]
    struct Ui {
        children: Vec<String>,
        labels: BTreeMap<String, String>,
        hidden: BTreeSet<String>,
    }
    impl CatalogUi for Ui {
        fn exists(&self, _: &str) -> bool {
            false
        }
        fn children(&self, _: &str) -> Vec<String> {
            self.children.clone()
        }
        fn text(&self, p: &str) -> Option<String> {
            self.labels.get(p).cloned()
        }
        fn hide(&mut self, p: &str) {
            self.hidden.insert(p.to_owned());
        }
    }
    #[test]
    fn all_thirty_stock_cards_hide_while_mlbb_and_tier_headers_remain() {
        let mut ui = Ui::default();
        for (i, (_, key)) in crate::item_catalog::GAME_ITEM_NAMES.iter().enumerate() {
            let node = format!("card{i}");
            ui.children.push(node.clone());
            ui.labels.insert(
                format!("list.{node}.name"),
                format!("#asset/base/text/item?{key}.name"),
            );
        }
        for key in [
            "tier_5_header",
            "radiant_armor",
            "mythical_radiant_armor",
            "warcrest_blade",
            "swift_boots",
        ] {
            ui.children.push(key.into());
        }
        filter_cards(&mut ui, "list");
        assert_eq!(ui.hidden.len(), 30);
        for key in [
            "tier_5_header",
            "radiant_armor",
            "mythical_radiant_armor",
            "warcrest_blade",
            "swift_boots",
        ] {
            assert!(!ui.hidden.contains(&format!("list.{key}")));
        }
    }
    #[test]
    fn rebuilt_catalog_cards_are_filtered_again_without_touching_other_panels() {
        let mut ui = Ui::default();
        ui.children = vec!["native".into(), "mlbb".into()];
        ui.labels.insert(
            "list.native.name".into(),
            "[TFM2] Warlords Final Judgement".into(),
        );
        ui.labels
            .insert("list.mlbb.name".into(), "Mythical Berserker's Fury".into());
        filter_cards(&mut ui, "list");
        ui.hidden.clear();
        filter_cards(&mut ui, "list");
        assert_eq!(ui.hidden, BTreeSet::from(["list.native".into()]));
    }
    #[test]
    fn names_are_recognized_exactly_without_matching_mlbb_radiant_armor() {
        assert!(stock_label("#asset/base/text/item?ironsword.name"));
        assert!(!stock_label("#asset/base/text/item?radiant_armor.name"));
        assert!(!stock_label(
            "#asset/base/text/item?mythical_radiant_armor.name"
        ));
        assert!(!stock_key("mlbb_dagger"));
        assert!(stock_key("dagger"));
    }

    #[derive(Default)]
    struct Tree {
        nodes: std::collections::BTreeSet<String>,
        hidden: std::collections::BTreeSet<String>,
    }
    impl Tree {
        fn add(&mut self, path: &str) {
            let mut parts = Vec::new();
            for part in path.split('.') {
                parts.push(part);
                self.nodes.insert(parts.join("."));
            }
        }
    }
    impl CatalogUi for Tree {
        fn exists(&self, path: &str) -> bool {
            self.nodes.contains(path)
        }
        fn children(&self, path: &str) -> Vec<String> {
            let prefix = format!("{path}.");
            self.nodes
                .iter()
                .filter_map(|node| node.strip_prefix(&prefix))
                .filter(|child| !child.contains('.'))
                .map(str::to_owned)
                .collect()
        }
        fn text(&self, _: &str) -> Option<String> {
            None
        }
        fn hide(&mut self, path: &str) {
            self.hidden.insert(path.to_owned());
        }
    }
    #[test]
    fn game_info_and_nested_builds_popup_are_discovered_and_filtered_together() {
        let mut ui = Tree::default();
        let pages = [
            "main.top.right.game_info.item_info",
            "main.item_info_popup.popup.content.item_info",
            "main.contents.item_info",
        ];
        for page in pages {
            let list = format!("{page}.data.item_list.contents");
            for (_, key) in crate::item_catalog::GAME_ITEM_NAMES {
                ui.add(&format!("{list}.{key}.name"));
            }
            ui.add(&format!("{list}.radiant_armor.name"));
            ui.add(&format!("{list}.tier_4_header.text"));
        }
        // A stock item in the HUD must never be hidden.
        ui.add("main.item_tooltip.data.ironsword.name");
        let found = discover(&ui);
        assert_eq!(found.len(), 3);
        for path in &found {
            filter_cards(&mut ui, path);
        }
        assert_eq!(ui.hidden.len(), 90);
        assert!(!ui.hidden.iter().any(|p| p.ends_with("radiant_armor")
            || p.ends_with("tier_4_header")
            || p.contains("item_tooltip")));
        // A newly mounted page is still found while the popup survives.
        ui.add("main.top.right.other.item_info.data.item_list.contents.mlbb_dagger");
        assert_eq!(discover(&ui).len(), 4);
    }
}

#[cfg(test)]
mod recipe_lifecycle {
    use super::*;
    use mod_api_stable::{ClientCtxV1, UiVtableV1};
    use std::{collections::BTreeMap, ffi::c_void, mem::size_of};

    const PAGE: &str = "main.top.right.game_info.item_info";
    const DETAIL: &str = "main.top.right.game_info.item_info.data.item_detail";
    const CATALOG: &str = "main.top.right.game_info.item_info.data.item_list.contents";

    struct Host {
        visible: BTreeMap<String, bool>,
        spawned_recipe_pane: bool,
        top: f32,
        spawns: usize,
        body_spawns: usize,
        selected: &'static str,
        reject_body: bool,
        stat_lines: String,
    }
    unsafe fn borrowed<'a>(pointer: *const c_void, len: usize) -> &'a str {
        let path = std::slice::from_raw_parts(pointer.cast(), len);
        std::str::from_utf8(path).unwrap()
    }
    unsafe fn host<'a>(state: *const c_void) -> &'a mut Host {
        &mut *state.cast_mut().cast()
    }
    unsafe fn string(value: &str, buf: *mut u8, cap: usize, len: *mut usize) -> bool {
        *len = value.len();
        if !buf.is_null() && cap >= value.len() {
            std::ptr::copy_nonoverlapping(value.as_ptr(), buf, value.len());
        }
        true
    }
    unsafe extern "C" fn child_count(_: *const c_void, p: *const u8, n: usize, out: *mut usize) -> bool {
        *out = usize::from(matches!(borrowed(p.cast(),n), "main.top.right" | "main.top.right.game_info"));
        true
    }
    unsafe extern "C" fn child_name(_: *const c_void, p: *const u8, n: usize, _: usize, buf: *mut u8, cap: usize, out: *mut usize) -> bool {
        let name = match borrowed(p.cast(), n) {
            "main.top.right" => "game_info",
            "main.top.right.game_info" => "item_info",
            _ => return false,
        };
        string(name, buf, cap, out)
    }
    unsafe extern "C" fn exists(s: *const c_void, p: *const u8, n: usize) -> bool {
        let path = borrowed(p.cast(),n);
        path == CATALOG || path == PAGE || host(s).visible.contains_key(path)
            || (path.ends_with(RECIPE_PANE) && host(s).spawned_recipe_pane)
    }
    unsafe extern "C" fn get_visible(s: *const c_void, p: *const u8, n: usize, out: *mut bool) -> bool {
        let Some(visible) = host(s).visible.get(borrowed(p.cast(),n)) else { return false; };
        *out = *visible;
        true
    }
    unsafe extern "C" fn set_visible(s: *mut c_void, p: *const u8, n: usize, visible: bool) -> bool {
        host(s).visible.insert(borrowed(p.cast(),n).to_owned(), visible);
        true
    }
    unsafe extern "C" fn get_text(s: *const c_void, p: *const u8, n: usize, buf: *mut u8, cap: usize, out: *mut usize) -> bool {
        if borrowed(p.cast(),n) != format!("{DETAIL}.name") { return false; }
        string(host(s).selected,buf,cap,out)
    }
    unsafe extern "C" fn set_text(s: *mut c_void, path: *const u8, n: usize, text: *const u8, t: usize) -> bool {
        if borrowed(path.cast(), n).ends_with(".stats") {
            host(s).stat_lines = borrowed(text.cast(), t).to_owned();
        }
        true
    }
    unsafe extern "C" fn layout(s: *const c_void, p: *const u8, n: usize, _: bool, x: *mut f32, y: *mut f32, w: *mut f32, h: *mut f32) -> bool {
        let path = borrowed(p.cast(),n);
        *x = 0.0;
        *w = 540.0;
        let (py, ph) = match path {
            DETAIL => (0.0, 900.0),
            p if p.ends_with(".name") => (42.0, 38.0),
            p if p.ends_with(".desc") => (host(s).top, 440.0),
            _ => return false,
        };
        *y = py;
        *h = ph;
        true
    }
    unsafe extern "C" fn spawn(s: *mut c_void, path: *const u8, n: usize, _: *const u8, _: usize) -> bool {
        let h = host(s);
        if borrowed(path.cast(), n).ends_with(RECIPE_PANE) {
            if h.reject_body { return false; }
            h.body_spawns += 1;
        } else {
            h.spawned_recipe_pane = true;
            h.spawns += 1;
        }
        true
    }
    unsafe extern "C" fn remove(s: *mut c_void, path: *const u8, n: usize) -> bool {
        if borrowed(path.cast(), n).ends_with(RECIPE_PANE) {
            host(s).spawned_recipe_pane = false;
        }
        true
    }
    #[test]
    fn recipe_ownership_hides_native_layers_between_redraws_and_preserves_scroll_runner() {
        let mut h = Host {
            visible: BTreeMap::new(), spawned_recipe_pane: false, top: 110.0, spawns: 0,
            body_spawns: 0, selected: "#asset/base/text/item?ares_belt.name",
            reject_body: false, stat_lines: String::new(),
        };
        h.visible.insert(DETAIL.into(), true);
        let native = NATIVE_DETAIL.map(|node| format!("{DETAIL}.{node}"));
        for node in &native { h.visible.insert(node.clone(), true); }
        let mut ui: UiVtableV1 = unsafe { std::mem::zeroed() };
        ui.size=size_of::<UiVtableV1>(); ui.query_exists=Some(exists);
        ui.child_count=Some(child_count); ui.child_name_at=Some(child_name);
        ui.get_visible=Some(get_visible); ui.set_visible=Some(set_visible);
        ui.get_text=Some(get_text); ui.set_text=Some(set_text); ui.node_rect=Some(layout);
        ui.spawn_source=Some(spawn); ui.remove_node=Some(remove);
        let mut raw: ClientCtxV1 = unsafe { std::mem::zeroed() };
        raw.size=size_of::<ClientCtxV1>(); raw.ui=&ui; raw.state=(&mut h as *mut Host).cast();
        let mut ctx=unsafe { StableClient::from_raw(&mut raw) }.unwrap();
        sync(&mut ctx);
        assert!(h.spawned_recipe_pane);
        assert!(native.iter().all(|node| h.visible[node] == false));
        assert!(h.stat_lines.contains("300"));
        for _ in 0..6 {
            for node in &native { h.visible.insert(node.clone(), true); }
            sync(&mut ctx);
            assert!(h.spawned_recipe_pane);
            assert!(native.iter().all(|node| !h.visible[node]));
        }
        assert_eq!(h.spawns, 1);
        h.top = 0.0;
        for _ in 0..6 { sync(&mut ctx); }
        assert_eq!(h.spawns, 1);
        assert_eq!(h.body_spawns, 1);

        h.selected = "#asset/base/text/item?corrosion_scythe.name";
        sync(&mut ctx);
        assert_eq!(h.spawns, 1);
        assert_eq!(h.body_spawns, 2);
        assert_eq!(h.stat_lines.lines().count(), 3);

        h.selected = "#asset/base/text/item?endless_battle.name";
        h.reject_body = true;
        sync(&mut ctx);
        assert!(!h.spawned_recipe_pane);
        assert!(native.iter().all(|node| h.visible[node]));
    }
}
