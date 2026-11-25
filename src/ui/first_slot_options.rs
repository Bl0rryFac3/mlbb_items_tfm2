//! Global first-slot switches next to the Mythical rule, also in Options.
use mod_api_stable::{StableClient, UiEventKindV1};
use std::{collections::BTreeMap, sync::Mutex};

const NODE: &str = "mlbb_first_slots";

fn source(footer: bool) -> String {
    let (placement, start, width, gap) = if footer {
        ("x:15px; y:56px; width:730px; height:38px;", 0, 351, 10)
    } else {
        ("width:1160px; height:44px;", 255, 420, 12)
    };
    let second = start + width + gap;
    let title = if footer { String::new() } else {
        r#"#header:label {
          @"asset/base/style/main#label"; width:240px; height:36px;
          align_y:Center; size:16; text:"First-slot rules"; ignore_event:true;
        }"#.to_owned()
    };
    format!(r#"{NODE}:empty {{ {placement} {title}
      #jungle:color_selectable {{
        @"asset/base/style/main#strategy_option"; x:{start}px; width:{width}px; height:36px;
        text:"Force Jungle First: On";
      }}
      #roam:color_selectable {{
        @"asset/base/style/main#strategy_option"; x:{second}px; width:{width}px; height:36px;
        text:"Force Roam Boots First: On";
      }}
    }}"#)
}

fn locked(ctx: &StableClient<'_>) -> bool {
    crate::multiplayer::host_first_slot_rules().is_some() || super::options_row::locked_now(ctx)
}

fn event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else { return };
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) || locked(ctx) {
        return;
    }
    let mut settings = crate::first_slot_rules::current();
    if event.path.ends_with(".mlbb_first_slots.jungle") {
        settings.jungle_first = !settings.jungle_first;
    } else if event.path.ends_with(".mlbb_first_slots.roam") {
        settings.roam_first = !settings.roam_first;
    } else {
        return;
    }
    if !crate::first_slot_rules::choose(settings) {
        crate::hook::build_status("first-slot/save", "Cannot write first-slot-rules.json; setting retained".into());
    }
}

pub(super) fn sync(ctx: &mut StableClient<'_>, parent: &str, footer: bool, visible: bool) {
    static PAINTED: Mutex<BTreeMap<String, u8>> = Mutex::new(BTreeMap::new());

    let row = format!("{parent}.{NODE}");
    let fresh = !ctx.ui_exists(&row);
    if fresh {
        if !visible || !ctx.ui_spawn_source(parent, &source(footer)) {
            return;
        }
        for cell in ["jungle", "roam"] {
            ctx.ui_register_path_events(&format!("{row}.{cell}"), event);
        }
    }
    ctx.ui_set_visible(&row, visible);
    if !visible {
        return;
    }
    let settings = crate::first_slot_rules::current();
    let locked = locked(ctx);
    let appearance = settings.code() | (u8::from(locked) << 2);
    if let Ok(mut painted) = PAINTED.lock() {
        if painted.len() > 8 {
            painted.clear();
        }
        if painted.insert(row.clone(), appearance) == Some(appearance) && !fresh {
            return;
        }
    }
    let hint = if locked {
        "Change between matches. Multiplayer uses the host's settings."
    } else {
        "Force eligible role gear into slot 1 in new matches. Gear set to None stays disabled."
    };
    let size = if footer { 14 } else { 16 };
    for (cell, on, title) in [
        ("jungle", settings.jungle_first, "Force Jungle First"),
        ("roam", settings.roam_first, "Force Roam Boots First"),
    ] {
        let style = if locked {
            crate::strategy_ui::toggle_off_style_sized(on, size)
        } else {
            crate::strategy_ui::toggle_style_sized(on, size)
        };
        let state = if on { "On" } else { "Off" };
        ctx.ui_set_properties(&format!("{row}.{cell}"),
            &format!(r#"{style} text:"{title}: {state}"; hint:"{hint}";"#));
    }
}
