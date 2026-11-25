//! Named alternatives for a champion and role, separate from active builds.
use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};

use crate::build_config::{self, ChampionRow, Focus, Role};

const FORMAT: u32 = 1;
const MAX_PLANS: usize = 600;
const MAX_ITEMS: usize = 6;

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Plan {
    pub name: String,
    build: String,
    slots: [Option<String>; MAX_ITEMS],
    #[serde(default)]
    focus: Option<String>,
    #[serde(default)]
    gear: crate::role_gear::Choice,
}

impl Plan {
    pub fn new(name: &str, row: &ChampionRow) -> Result<Self, &'static str> {
        let champion = row.champion.as_deref().ok_or("Choose a champion first.")?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err("Enter a name between 1 and 40 characters.");
        }
        if !row.is_complete() {
            return Err("Add at least one item or role setting before saving.");
        }
        Ok(Self {
            name: name.to_owned(),
            build: build_config::build_key(champion, row.role),
            slots: std::array::from_fn(|slot| row.slots.get(slot).cloned().flatten()),
            focus: row.focus.stored().map(str::to_owned),
            gear: row.gear,
        })
    }

    pub fn row(&self) -> ChampionRow {
        let (champion, role) = build_config::split_build_key(&self.build);
        ChampionRow {
            champion: Some(champion.to_owned()),
            role,
            slots: self.slots.to_vec(),
            focus: self.focus.as_deref().map_or(Focus::Neither, Focus::from_stored),
            gear: self.gear,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Library {
    format: u32,
    plans: Vec<Plan>,
    #[serde(default)]
    favorites: BTreeSet<String>,
}

fn path(file: &str) -> PathBuf {
    crate::config::mod_dir().join(file)
}

impl Library {
    pub fn load() -> Result<Self, &'static str> {
        Self::read(&path("build-library.json"), true)
    }

    pub fn import() -> Result<Self, &'static str> {
        Self::read(&path("build-library-import.json"), false)
    }

    fn read(file: &Path, allow_missing: bool) -> Result<Self, &'static str> {
        let bytes = match fs::read(file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_missing => {
                return Ok(Self { format: FORMAT, plans: Vec::new(), favorites: BTreeSet::new() });
            }
            Err(_) => return Err("Could not read the build library file."),
        };
        let library: Self = serde_json::from_slice(&bytes).map_err(|_| "The build library JSON is invalid.")?;
        if library.format != FORMAT || library.plans.len() > MAX_PLANS || library.favorites.len() > 512 {
            return Err("This build library version or size is unsupported.");
        }
        if library.plans.iter().any(|plan| {
            Plan::new(&plan.name, &plan.row()).is_err()
                || plan.slots.iter().flatten().any(|key| key.len() > 128 || key.chars().any(char::is_control))
        }) {
            return Err("The build library contains an invalid plan.");
        }
        Ok(library)
    }

    pub fn matching(&self, champion: &str, role: Role) -> impl Iterator<Item = &Plan> {
        let key = build_config::build_key(champion, role);
        self.plans.iter().filter(move |plan| plan.build == key)
    }

    pub fn append(&mut self, plan: Plan) -> Result<(), &'static str> {
        if self.plans.iter().any(|saved| saved.build == plan.build && saved.name.eq_ignore_ascii_case(&plan.name)) {
            return Err("That name is already used for this champion and role.");
        }
        if self.plans.len() >= MAX_PLANS {
            return Err("The build library is full.");
        }
        self.plans.push(plan);
        Ok(())
    }

    pub fn merge(&mut self, incoming: Self) -> (usize, usize) {
        let total = incoming.plans.len();
        let mut added = 0;
        for key in incoming.favorites {
            if self.favorites.len() < 512 && crate::mlbb::statistics_final_keys().contains(&key) {
                self.favorites.insert(key);
            }
        }
        for plan in incoming.plans {
            added += usize::from(self.append(plan).is_ok());
        }
        (added, total - added)
    }

    pub fn favorites(&self) -> &BTreeSet<String> {
        &self.favorites
    }

    pub fn toggle_favorite(&mut self, key: &str) -> bool {
        if self.favorites.remove(key) {
            return false;
        }
        self.favorites.insert(key.to_owned());
        true
    }

    pub fn save(&self) -> Result<(), &'static str> {
        self.write(&path("build-library.json"))
    }

    pub fn export(&self) -> Result<(), &'static str> {
        self.write(&path("build-library-export.json"))
    }

    fn write(&self, file: &Path) -> Result<(), &'static str> {
        let encoded = serde_json::to_vec_pretty(self).map_err(|_| "Could not encode the build library.")?;
        let temporary = file.with_extension("json.tmp");
        fs::write(&temporary, encoded).and_then(|_| fs::rename(&temporary, file))
            .map_err(|_| "Could not save the build library.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternatives_stay_scoped_and_merge_keeps_existing_names() {
        let row = ChampionRow { champion: Some("mlbb_karrie".into()), role: Role::Bottom,
            slots: vec![Some("golden_staff".into())], ..Default::default() };
        let saved = Plan::new("On hit", &row).unwrap();
        let mut library = Library { format: FORMAT, plans: vec![saved.clone()], favorites: BTreeSet::new() };
        let mut other = row.clone();
        other.role = Role::Top;
        let alternate = Plan::new("On hit", &other).unwrap();
        assert_eq!(library.merge(Library { format: FORMAT, plans: vec![saved, alternate], favorites: BTreeSet::new() }), (1, 1));
        assert_eq!(library.matching("mlbb_karrie", Role::Bottom).next().unwrap().row().slots[0], row.slots[0]);
        assert_eq!(library.matching("mlbb_karrie", Role::Top).count(), 1);
    }
}
