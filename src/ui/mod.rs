//! In-game UI the mod adds or reshapes: the Builds tab (`strategy_ui`), the
//! solo-rank match history's item row, the Check Tactics builds
//! (`match_builds`), match tooltips (`item_spend`) and icon counts
//! (`item_counts`), and two Options dialog rows (`options_row`,
//! `patch_settings`). The Item Stats tab lives in `item_stats::ui`.

pub(crate) mod catalog_filter;
pub(crate) mod draft_watch;
pub(crate) mod item_counts;
pub(crate) mod item_spend;
pub(crate) mod match_builds;
pub(crate) mod roam_ui;
mod name_search;
mod options_dialog;
pub(crate) mod options_row;
mod first_slot_options;
pub(crate) mod build_stats;
pub(crate) mod patch_settings;
pub(crate) mod solo_rank_ui;
pub(crate) mod strategy_ui;

use std::collections::BTreeSet;
use std::sync::Mutex;

use mod_api_stable::StableClient;

pub(super) fn active_champion_ids(ctx: &StableClient<'_>) -> Vec<String> {
    let mut names = ctx.champion_names();
    names.retain(|key| !key.trim().is_empty());
    let from_client = !names.is_empty();
    if !from_client {
        // An empty client sheet can mean that the management data is still
        // loading. The scorer snapshot may only belong to this career.
        names = crate::build_config::current_champion_roster();
    }
    let reported = names.len();
    let resolved: Vec<_> = names.iter().filter(|key| ctx.champion_brief(key).is_some()).cloned().collect();
    // Some hosts have the roster snapshot before the champion lookup is ready.
    // Once this host can resolve champions, omit stale IDs in either list.
    if !resolved.is_empty() {
        names = resolved;
    }
    let source = if from_client { "client sheet" } else { "current career scorer" };
    if reported != 0 {
        crate::patches::log("build_stats.roster", || format!("{source}: {} of {reported} IDs available", names.len()));
    }
    names
}

/// A complete text block for a native color_icon_button. Runtime property
/// writes replace this block, so the font and alignment travel with its text.
pub(super) fn button_text(label: &str, size: usize, left: bool) -> String {
    let label = options_dialog::escape(label);
    let align = if left { "Left" } else { "Center" };
    format!(
        r#"text: {{font:"asset/base/font/set/bold"; text:"{label}";
        size:{size}; align_x:{align}; align_y:Center; fit_width:true;}}"#
    )
}

// The paths a screen has registered its UI handler on. A handler is keyed by
// the path string and lives until process exit, so registering a path twice
// fires one click twice; each screen registers through its set and forgets it
// when the screen goes away, not before.
pub(crate) struct Registered {
    paths: Mutex<BTreeSet<String>>,
    handler: fn(&mut StableClient<'_>),
}

impl Registered {
    pub(crate) const fn new(handler: fn(&mut StableClient<'_>)) -> Self {
        Self {
            paths: Mutex::new(BTreeSet::new()),
            handler,
        }
    }

    // Registers the handler for `path` unless it has one; a path the host
    // refuses is forgotten, so a later call retries.
    pub(crate) fn register(&self, ctx: &mut StableClient<'_>, path: &str) {
        let fresh = self
            .paths
            .lock()
            .map(|mut set| set.insert(path.to_string()))
            .unwrap_or(false);
        if !fresh {
            return;
        }
        if !ctx.ui_register_path_events(path, self.handler) {
            if let Ok(mut set) = self.paths.lock() {
                set.remove(path);
            }
        }
    }

    // Forgets every path: the screen, and every node registered on it, is
    // gone.
    pub(crate) fn forget(&self) {
        if let Ok(mut set) = self.paths.lock() {
            set.clear();
        }
    }
}
