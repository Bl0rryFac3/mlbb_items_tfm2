use mod_api_stable::StableClient;
use std::collections::BTreeMap;

use super::{clean, fill, handle_click, TextBox, Window};
use crate::{passive_presets::Preset, passive_tuning, patches::text};

const PANEL: &str = r##"passive:empty {
  y:76px; width:1404px; height:816px;
  #variant:color_icon_button {
    @"asset/base/style/main#secondary_button";
    width:660px; height:44px;
    text:{text:"%VARIANT%"; size:17; align_x:Center; align_y:Center;}
    hint:"Choose the current passive or a supported historical variant.";
  }
  #tier:color_icon_button {
    @"asset/base/style/main#secondary_button";
    x:736px; width:240px; height:44px;
    text:{text:"%TIER%"; size:17; align_x:Center; align_y:Center;}
    hint:"Preview the selected tier. Edited values are shared by both tiers.";
  }
  #help:label {
    @"asset/base/style/main#label"; y:54px; width:670px; height:64px;
    size:16; line_height:21; color:#aab5c5ff;
  }
  #fields:scroll_view {
    y:125px; width:681px; height:454px; speed:90; bar_width:4;
    bar:{source:"asset/base/sprite/white";color:#53dcc8ff;}
    back:{source:"asset/base/sprite/white";color:#394452ff;}
    #rows:empty {width:670px;height:1px;}
  }
  #preview_card:color {
    x:736px; y:56px; width:660px; height:513px; color:#171c25ff;
    #heading:label {
      @"asset/base/style/main#bold_label"; x:16px; y:10px; width:620px; height:28px;
      size:19; text:"Live preview"; color:#53dcc8ff;
    }
    #preview:label {
      @"asset/base/style/main#label"; x:16px; y:51px; width:628px; height:440px;
      size:18; line_height:25;
    }
  }
  #status:label {
    @"asset/base/style/main#label"; x:0px; y:596px; width:1370px; height:44px;
    size:17; color:#aab5c5ff;
  }
  #revert:color_icon_button {
    @"asset/base/style/main#secondary_button"; y:658px; width:196px; height:44px;
    text:{text:"Revert draft"; size:16; align_x:Center; align_y:Center;}
  }
  #reset:color_icon_button {
    @"asset/base/style/main#secondary_button"; x:208px; y:658px; width:220px; height:44px;
    text:{text:"Reset to default"; size:16; align_x:Center; align_y:Center;}
  }
  #apply:color_icon_button {
    @"asset/base/style/main#primary_button"; x:440px; y:658px; width:220px; height:44px;
    text:{text:"Apply changes"; size:16; color:#141820ff; align_x:Center; align_y:Center;}
  }
  #undo:color_icon_button {
    @"asset/base/style/main#tertiary_button"; x:736px; y:658px; width:240px; height:44px;
    text:{text:"Undo saved change"; size:16; align_x:Center; align_y:Center;}
    hint:"Restore the previous passive settings for this career.";
  }
}"##;

const FIELD: &str = r##"v%INDEX%:empty {
  y:%Y%px; width:670px; height:55px;
  #name:label {
    @"asset/base/style/main#label"; width:398px; height:32px;
    align_y:Center; size:17; text:"%LABEL%";
  }
  #bounds:label {
    @"asset/base/style/main#label"; y:33px; width:398px; height:17px;
    size:12; color:#aab5c5ff; text:"%BOUNDS%";
  }
  #value:text_edit {
    @"asset/base/style/main#text_edit"; x:408px; width:240px; height:42px; size:17;
    align_y:Center; padding:{left:12px; top:4px; right:12px; bottom:4px;}
    placeholder:"%VALUE%"; max_length:24;
  }
}"##;

#[derive(Clone, Copy)]
enum Destination {
    Page,
    Item(usize),
    Variant(Preset),
}

#[derive(Default)]
pub(super) struct Editor {
    pub(super) shown: bool,
    key: String,
    historical: Preset,
    initial_variant: Preset,
    initial_current: BTreeMap<String, f64>,
    inputs: Vec<NumberInput>,
    mythical: bool,
    dropdown: bool,
    reset_current: bool,
    pending: Option<Destination>,
}

struct NumberInput {
    box_: TextBox,
    starting: f64,
    value: Option<f64>,
}

fn read_number(typed: &str, starting: f64) -> Option<f64> {
    let typed = typed.trim();
    if typed.is_empty() { Some(starting) } else { typed.parse().ok() }
}

impl Editor {
    fn load(&mut self, key: String) {
        self.historical = crate::passive_presets::equipped(&key);
        self.initial_variant = self.historical;
        self.initial_current = passive_tuning::values(&key);
        self.key = key;
        self.mythical = false;
        self.dropdown = false;
        self.pending = None;
        self.reset_current = false;
    }

    fn variants(&self) -> Vec<Preset> {
        let mut options = vec![Preset::Default];
        let mut next = Preset::Default.next(&self.key);
        while next != Preset::Default {
            options.push(next);
            next = next.next(&self.key);
        }
        options
    }

    fn edited(&self) -> bool {
        self.reset_current || self.historical != self.initial_variant
            || self.inputs.iter().any(|input| input.value != Some(input.starting))
    }

    fn values(&self) -> Option<Vec<f64>> {
        self.inputs.iter().map(|input| input.value).collect()
    }

    fn draft(&self) -> Option<(Preset, BTreeMap<String, f64>)> {
        let values = self.values()?;
        if self.historical != Preset::Default {
            return self.historical.with_values(&values).map(|preset| (preset, BTreeMap::new()));
        }
        let fields = passive_tuning::fields(&self.key);
        if fields.len() != values.len() {
            return None;
        }
        let mut saved = BTreeMap::new();
        for (field, value) in fields.iter().zip(values) {
            if !field.accepts(value) {
                return None;
            }
            if field.default != value {
                saved.insert(field.id.clone(), value);
            }
        }
        Some((Preset::Default, saved))
    }

    fn preview(&self) -> String {
        let Some((variant, settings)) = self.draft() else {
            return "<#f17b70ff>Enter a number within the shown limits.<>".into();
        };
        if let Some(text) = variant.description(self.mythical) {
            return text;
        }
        let key = if self.mythical { format!("mythical_{}", self.key) } else { self.key.clone() };
        let original = crate::patches::text::base_option(&key).unwrap_or("").trim();
        if original.is_empty() {
            return if self.key == "expert_gloves" {
                "Adaptive Force adds Attack or Magic Power according to the wielder’s stronger offensive stat.".into()
            } else {
                "This item has no separate passive description. Check its stats on the Item Stats tab.".into()
            };
        }
        passive_tuning::description(&key, original, &settings).unwrap_or_else(|| original.to_owned())
    }

    fn apply(&self) -> Result<(), &'static str> {
        let Some((preset, values)) = self.draft() else { return Err("Correct the highlighted value in the preview before applying."); };
        let edited = crate::patches::edit_settings(|state| {
            if preset == Preset::Default {
                state.passives.remove(&self.key);
                if values.is_empty() {
                    state.passive_tuning.remove(&self.key);
                } else {
                    state.passive_tuning.insert(self.key.clone(), values);
                }
            } else {
                state.passives.insert(self.key.clone(), preset);
            }
        });
        edited.then_some(()).ok_or("No career settings are available.")
    }
}

fn bind(ctx: &mut StableClient<'_>, parent: &str, id: &str) {
    ctx.ui_register_path_events(&format!("{parent}.{id}"), handle_click);
}

fn choice_buttons(ctx: &mut StableClient<'_>, panel: &str, editor: &Editor) {
    use std::fmt::Write;
    let path = format!("{panel}.choices");
    ctx.ui_remove_node(&path);
    if !editor.dropdown && editor.pending.is_none() { return; }
    let mut source = String::from("choices:color { x:0px; y:47px; width:665px; height:240px; color:#171c25ff;");
    let labels = if editor.pending.is_some() {
        vec![("keep".into(), "Keep editing"), ("discard".into(), "Discard draft"), ("apply_move".into(), "Apply and continue")]
    } else {
        editor.variants().iter().enumerate().map(|(i, variant)| {
            (format!("choice{i}"), if *variant == Preset::Default { "Current passive" } else { variant.label() })
        }).collect()
    };
    for (index, (id, label)) in labels.iter().enumerate() {
        let y = 8 + index * 45;
        let _ = write!(source, r##"#{id}:color_icon_button {{
          @"asset/base/style/main#secondary_button"; x:8px; y:{y}px; width:640px; height:40px;
          text:{{text:"{label}"; size:16; align_x:Center; align_y:Center;}}
        }}"##);
    }
    source.push('}');
    if ctx.ui_spawn_source(panel, &source) {
        for (id, _) in labels { bind(ctx, &path, &id); }
    }
}

fn render(ctx: &mut StableClient<'_>, window: &mut Window) {
    let main = window.under(".popup.main");
    let panel = format!("{main}.passive");
    ctx.ui_remove_node(&panel);
    ctx.ui_set_visible(&format!("{main}.page"), !window.passive.shown);
    ctx.ui_set_visible(&format!("{main}.stats_advanced"), !window.passive.shown);
    if window.passive.shown { ctx.ui_set_visible(&format!("{main}.stats_tier"), false); }
    let footer = window.under(".popup.footer");
    if !window.passive.shown {
        ctx.ui_set_text(&format!("{main}.passive_tab_text"), "Passive Editor");
        ctx.ui_set_text(&format!("{main}.subtitle"), super::STATS_HINT);
        ctx.ui_set_text(&format!("{footer}.only_text"), "Pending stat patches");
        return;
    }
    ctx.ui_set_text(&format!("{main}.passive_tab_text"), "Item Stats");
    ctx.ui_set_text(&format!("{main}.subtitle"), "Search by name, class, legacy, mythical, or tier:4");
    ctx.ui_set_text(&format!("{footer}.only_text"), "Modified passives");
    ctx.ui_set_visible(&format!("{footer}.queue"), false);
    ctx.ui_set_visible(&format!("{footer}.queue_off"), false);
    let name = window.picked.and_then(|i| window.entries.get(i)).map_or("", |e| e.name.as_str());
    let title = if name.is_empty() { "Passive Editor".into() } else { format!("{name}  /  Passives") };
    ctx.ui_set_text(&format!("{main}.title"), &title);
    let editor = &mut window.passive;
    editor.inputs.clear();
    let variant = if editor.historical == Preset::Default { "Current passive" } else { editor.historical.label() };
    let source = fill(PANEL, &[("%VARIANT%", variant), ("%TIER%", if editor.mythical { "Mythical preview" } else { "Base preview" })]);
    if !ctx.ui_spawn_source(&main, &source) { return; }
    for id in ["variant", "tier", "apply", "reset", "revert", "undo"] { bind(ctx, &panel, id); }

    let fields = if editor.historical == Preset::Default {
        passive_tuning::fields(&editor.key).iter().map(|field| {
            let value = if editor.reset_current { field.default } else { editor.initial_current.get(&field.id).copied().unwrap_or(field.default) };
            (field.label.as_str(), value, format!("{} to {}   Default {}", field.min, field.max, field.default))
        }).collect()
    } else {
        editor.historical.fields().into_iter().map(|(label, number)| (label, number, "Historical variant".into())).collect::<Vec<_>>()
    };
    ctx.ui_set_text(&format!("{panel}.help"), if fields.is_empty() {
        if matches!(editor.key.as_str(), "boots" | "arcane_boots" | "demon_shoes" | "magic_shoes" | "rapid_boots" | "swift_boots" | "tough_boots" | "warrior_boots")
            || crate::role_gear::is_jungle_key(&editor.key) {
            "Role gear uses shared rules. Set role and blessing in Builds. Check Item Stats for flat numbers."
        } else if text::base_option(&editor.key).is_none_or(|text| text.trim().is_empty()) {
            "This item has flat stats. Use Item Stats to edit them."
        } else {
            "This effect is read-only. Its mechanics need separate verification before adding settings."
        }
    } else {
        "Base and Mythical share settings. The preview shows the selected tier’s effective values."
    });
    let rows = format!("{panel}.fields.rows");
    ctx.ui_set_properties(&rows, &format!("height:{}px;", (fields.len() * 55).max(1)));
    for (index, (label, value, bounds)) in fields.into_iter().enumerate() {
        let field = fill(FIELD, &[
            ("%INDEX%", &index.to_string()), ("%Y%", &(index * 55).to_string()),
            ("%LABEL%", &clean(&label.replace("(%)", "(percent)"))), ("%VALUE%", &value.to_string()), ("%BOUNDS%", &bounds),
        ]);
        if !ctx.ui_spawn_source(&rows, &field) { break; }
        editor.inputs.push(NumberInput {
            box_: TextBox::filled(ctx, format!("{rows}.v{index}.value"), value.to_string()),
            starting: value,
            value: Some(value),
        });
    }
    let history = crate::patches::settings(|state| state.passive_history.len()).unwrap_or(0);
    ctx.ui_set_visible(&format!("{panel}.undo"), history > 0);
    ctx.ui_set_text(&format!("{panel}.preview_card.preview"), &editor.preview());
}

fn selected_key(window: &Window) -> String {
    window.picked.and_then(|i| window.entries.get(i)).map(|e| {
        e.keys.first().cloned().unwrap_or_else(|| e.id.clone())
    }).unwrap_or_default()
}

fn move_to(ctx: &mut StableClient<'_>, window: &mut Window, destination: Destination) -> bool {
    let changed = match destination {
        Destination::Page => {
            window.passive.shown = !window.passive.shown;
            window.passive.load(selected_key(window));
            window.queue_look = 0;
            true
        }
        Destination::Item(index) => {
            if let Some(before) = window.picked.replace(index) {
                ctx.ui_set_visible(&format!("{}.i{before}.hl", window.list()), false);
            }
            ctx.ui_set_visible(&format!("{}.i{index}.hl", window.list()), true);
            window.passive.load(selected_key(window));
            false
        }
        Destination::Variant(preset) => {
            window.passive.historical = preset;
            window.passive.dropdown = false;
            window.passive.reset_current = false;
            false
        }
    };
    render(ctx, window);
    changed
}

pub(super) fn sync(
    ctx: &mut StableClient<'_>, window: &mut Window, clicks: &[String],
    settled: bool, imported: bool, picked: Option<usize>,
) -> bool {
    let tab = window.under(".popup.main.passive_tab");
    let clicked_tab = settled && clicks.contains(&tab);
    if !window.passive.shown {
        return clicked_tab && move_to(ctx, window, Destination::Page);
    }
    if imported {
        window.passive.load(selected_key(window));
        render(ctx, window);
        return false;
    }
    let panel = window.under(".popup.main.passive");
    let clicked = |id: &str| settled && clicks.contains(&format!("{panel}.{id}"));
    let status = format!("{panel}.status");
    let mut dirty = false;
    for input in &mut window.passive.inputs {
        if let Some(typed) = input.box_.typed(ctx) {
            input.value = read_number(&typed, input.starting);
            dirty = true;
        }
    }
    if dirty {
        ctx.ui_set_text(&format!("{panel}.preview_card.preview"), &window.passive.preview());
        ctx.ui_set_text(&status, "");
    }
    if let Some(destination) = window.passive.pending {
        if clicked("choices.keep") {
            window.passive.pending = None;
            choice_buttons(ctx, &panel, &window.passive);
            ctx.ui_set_text(&status, "");
        } else if clicked("choices.discard") || clicked("choices.apply_move") {
            if clicked("choices.apply_move") {
                if super::super::options_row::locked_now(ctx) || crate::multiplayer::active() {
                    ctx.ui_set_text(&status, "Passive settings can be applied between matches in single-player.");
                    return false;
                }
                if let Err(message) = window.passive.apply() {
                    ctx.ui_set_text(&status, message);
                    return false;
                }
            }
            window.passive.pending = None;
            return move_to(ctx, window, destination);
        }
        return false;
    }
    if clicked("variant") {
        window.passive.dropdown = !window.passive.dropdown;
        choice_buttons(ctx, &panel, &window.passive);
        return false;
    }
    if clicked("tier") {
        let key = format!("mythical_{}", window.passive.key);
        if crate::mlbb::catalog_recipe(&key).is_some() {
            window.passive.mythical = !window.passive.mythical;
            let label = if window.passive.mythical { "Mythical preview" } else { "Base preview" };
            ctx.ui_set_properties(&format!("{panel}.tier"), &super::super::button_text(label, 17, false));
            ctx.ui_set_text(&format!("{panel}.preview_card.preview"), &window.passive.preview());
        }
    }
    if clicked("undo") {
        if window.passive.edited() {
            ctx.ui_set_text(&status, "Apply or revert the current draft before undoing a saved edit.");
        } else if super::super::options_row::locked_now(ctx) || crate::multiplayer::active() {
            ctx.ui_set_text(&status, "Undo is available between matches in single-player.");
        } else if crate::patches::undo_passive_settings() {
            window.passive.load(selected_key(window));
            render(ctx, window);
            ctx.ui_set_text(&status, "Restored the previous passive settings in this career.");
        }
        return false;
    }
    if clicked("revert") || clicked("reset") {
        window.passive.load(selected_key(window));
        if clicked("reset") {
            window.passive.historical = Preset::Default;
            window.passive.reset_current = true;
        }
        render(ctx, window);
        return false;
    }
    if clicked("apply") {
        if super::super::options_row::locked_now(ctx) || crate::multiplayer::active() {
            ctx.ui_set_text(&status, "Passive settings can be applied between matches in single-player.");
            return false;
        }
        match window.passive.apply() {
            Ok(()) => {
                window.passive.load(selected_key(window));
                render(ctx, window);
                ctx.ui_set_text(&status, "Applied to this career.");
                super::lay_list(ctx, window);
            }
            Err(message) => { ctx.ui_set_text(&status, message); }
        }
        return false;
    }
    let choice = window.passive.variants().into_iter().enumerate().find_map(|(i, variant)| {
        clicked(&format!("choices.choice{i}")).then_some(Destination::Variant(variant))
    });
    let destination = clicked_tab.then_some(Destination::Page).or(picked.map(Destination::Item)).or(choice);
    if let Some(destination) = destination {
        if window.passive.edited() {
            window.passive.dropdown = false;
            window.passive.pending = Some(destination);
            choice_buttons(ctx, &panel, &window.passive);
            ctx.ui_set_text(&status, "Apply or discard the current draft before switching.");
        } else {
            return move_to(ctx, window, destination);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_setting_keeps_the_shown_starting_value() {
        assert_eq!(read_number(" ", 20.), Some(20.));
        assert_eq!(read_number(" 1.5 ", 20.), Some(1.5));
        assert_eq!(read_number("invalid", 20.), None);
    }
}
