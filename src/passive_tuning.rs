//! Numeric settings for existing effects. Historical variants keep their save format.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::{atomic::{AtomicBool, Ordering}, OnceLock, RwLock};

pub(crate) type Settings = BTreeMap<String, BTreeMap<String, f64>>;

#[derive(Deserialize)]
pub(crate) struct Field {
    pub id: String,
    pub label: String,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub whole: bool,
    position: usize,
    #[serde(default)]
    mythical_position: Option<usize>,
    #[serde(default = "one")]
    mythical_scale: f64,
    #[serde(default)]
    mythical_round: bool,
    #[serde(default)]
    range_part: Option<usize>,
}
fn one() -> f64 { 1. }

impl Field {
    pub(crate) fn accepts(&self, value: f64) -> bool {
        value.is_finite() && (self.min..=self.max).contains(&value)
            && (!self.whole || value.fract() == 0.)
    }
}

fn registry() -> &'static BTreeMap<String, Vec<Field>> {
    static REGISTRY: OnceLock<BTreeMap<String, Vec<Field>>> = OnceLock::new();
    REGISTRY.get_or_init(|| serde_json::from_str(include_str!("../catalog/passive-fields.json"))
        .expect("validated passive field definitions"))
}

pub(crate) fn fields(key: &str) -> &'static [Field] {
    registry().get(key).map_or(&[], Vec::as_slice)
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIVE: RwLock<Settings> = RwLock::new(BTreeMap::new());

pub(crate) fn any_active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

pub(crate) fn validated(settings: &Settings) -> Settings {
    settings.iter().filter_map(|(key, values)| {
        let values: BTreeMap<_,_> = fields(key).iter().filter_map(|field| {
            let value = *values.get(&field.id)?;
            (field.accepts(value) && value != field.default).then(|| (field.id.clone(), value))
        }).collect();
        (!values.is_empty()).then(|| (key.clone(), values))
    }).collect()
}

pub(crate) fn sync(settings: &Settings) {
    if let Ok(mut live) = LIVE.write() {
        *live = validated(settings);
        ACTIVE.store(!live.is_empty(), Ordering::Release);
    }
}

pub(crate) fn value(key: &str, id: &str, default: f64) -> f64 {
    if !any_active() {
        return default;
    }
    LIVE.read().ok()
        .and_then(|map| map.get(key)?.get(id).copied())
        .unwrap_or(default)
}

pub(crate) fn values(key: &str) -> BTreeMap<String, f64> {
    if !any_active() {
        return BTreeMap::new();
    }
    LIVE.read().ok().and_then(|map| map.get(key).cloned()).unwrap_or_default()
}

pub(crate) fn description(key: &str, base_option: &str, values: &BTreeMap<String, f64>) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let base = key.strip_prefix("mythical_").unwrap_or(key);
    let mythical = key != base;
    let mut replacements = BTreeMap::new();
    for field in fields(base) {
        if let Some(value) = values.get(&field.id).copied().filter(|v| field.accepts(*v)) {
            let value = if mythical { value * field.mythical_scale } else { value };
            let value = if mythical && field.mythical_round { value.round() } else { value };
            let position = if mythical { field.mythical_position.unwrap_or(field.position) } else { field.position };
            replacements.entry(position).or_insert_with(Vec::new).push((field.range_part, value));
        }
    }
    if replacements.is_empty() {
        return None;
    }

    const TAG: &str = "<#e8a800ff>";
    let mut output = String::with_capacity(base_option.len() + 12);
    let mut cursor = base_option;
    let mut found = 0;
    while let Some(from) = cursor.find(TAG) {
        let start = from + TAG.len();
        let end = start + cursor[start..].find("<>")?;
        output.push_str(&cursor[..start]);
        let existing = &cursor[start..end];
        if let Some(values) = replacements.remove(&found) {
            if values.iter().all(|(part, _)| part.is_some()) {
                let (low, high) = existing.split_once('–')?;
                let mut parts = [low.to_owned(), high.to_owned()];
                for (part, value) in values {
                    let slot = parts.get_mut(part?)?;
                    *slot = render_numeric(slot, value);
                }
                output.push_str(&parts.join("–"));
            } else {
                output.push_str(&render_numeric(existing, values.first()?.1));
            }
        } else {
            output.push_str(existing);
        }
        cursor = &cursor[end..];
        found += 1;
    }
    if !replacements.is_empty() {
        return None;
    }
    output.push_str(cursor);
    Some(output)
}

fn render_numeric(existing: &str, value: f64) -> String {
    let prefix = if existing.starts_with('+') { "+" } else { "" };
    let suffix = existing.chars().last().filter(|c| matches!(c, '%' | 's'));
    let body = format!("{value:.4}");
    let mut rendered = format!("{prefix}{}", body.trim_end_matches('0').trim_end_matches('.'));
    rendered.extend(suffix);
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_fields_address_valid_numeric_spans_in_every_shipped_locale() {
        let source: serde_json::Value = serde_json::from_str(include_str!("../text/item.i18n")).unwrap();
        for (language, entries) in source.as_object().unwrap() {
            for (key, fields) in registry() {
                for candidate in [key.clone(), format!("mythical_{key}")] {
                    let Some(original) = entries[&candidate]["option"].as_str() else { continue; };
                    for field in fields {
                        assert!(field.accepts(field.default), "{key}/{}", field.id);
                        let inputs = [(field.id.clone(), field.default)].into_iter().collect();
                        let rendered = description(&candidate, original, &inputs);
                        assert!(rendered.is_some(), "{language}/{candidate}/{}", field.id);
                    }
                }
            }
        }
    }

    #[test]
    fn modifies_only_the_addressed_effect_number_on_each_tier() {
        let source: serde_json::Value = serde_json::from_str(include_str!("../text/item.i18n")).unwrap();
        let changes = [("lifesteal".to_owned(), 20.)].into_iter().collect();
        for (key, expected) in [("concentrated_energy", "20%"), ("mythical_concentrated_energy", "25%")] {
            let original = source["en"][key]["option"].as_str().unwrap();
            let result = description(key, original, &changes).unwrap();
            assert!(result.contains(&format!("<#e8a800ff>{expected}<>")));
            assert!(result.contains("deals <#e8a800ff>12%<> more"));
        }
    }
}
