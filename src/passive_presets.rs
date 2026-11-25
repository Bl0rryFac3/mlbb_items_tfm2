//! Career-scoped variants for the item families supported by Item Patcher.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::RwLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub(crate) enum Preset {
    #[default]
    Default,
    Clock {
        interval: f64,
        hp: u16,
        magic: u16,
        stacks: u8,
        bonus: u8,
    },
    Thunder {
        window: f64,
        damage: u16,
        hp_percent: f64,
        slow: u8,
        max_slow: u8,
        slow_scale: f64,
        duration: f64,
        cooldown: f64,
    },
    Defiance {
        threshold: u16,
        reduction: u8,
        hp_percent: f64,
        seconds: u8,
        cooldown: f64,
    },
    Twilight {
        threshold: u16,
        scale: f64,
    },
    TwilightOnHit {
        interval: f64,
        damage: u16,
        hp_percent: f64,
    },
}

impl Preset {
    pub(crate) fn next(self, key: &str) -> Self {
        match (key, self) {
            ("clock_of_destiny", Self::Default) => Self::Clock {
                interval: 20.,
                hp: 25,
                magic: 4,
                stacks: 15,
                bonus: 5,
            },
            ("thunder_belt", Self::Default) => Self::Thunder {
                window: 3.,
                damage: 50,
                hp_percent: 5.,
                slow: 40,
                max_slow: 80,
                slow_scale: 1.,
                duration: 1.,
                cooldown: 1.5,
            },
            ("twilight_armor", Self::Default) => Self::Defiance {
                threshold: 600,
                reduction: 20,
                hp_percent: 100.,
                seconds: 3,
                cooldown: 6.,
            },
            ("twilight_armor", Self::Defiance { .. }) => Self::Twilight {
                threshold: 800,
                scale: 0.4,
            },
            ("twilight_armor", Self::Twilight { .. }) => Self::TwilightOnHit {
                interval: 1.5,
                damage: 35,
                hp_percent: 5.,
            },
            _ => Self::Default,
        }
    }

    pub(crate) fn supported(key: &str) -> bool {
        matches!(key, "clock_of_destiny" | "thunder_belt" | "twilight_armor")
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Clock { .. } => "Time and Reincarnate (1.7.44)",
            Self::Thunder { .. } => "Thunderbolt (1.5.88)",
            Self::Defiance { .. } => "Defiance (1.5.88)",
            Self::Twilight { .. } => "Twilight (1.8.92)",
            Self::TwilightOnHit { .. } => "Twilight (legacy on-hit)",
        }
    }

    pub(crate) fn preview(self) -> String {
        self.description(false)
            .unwrap_or_else(|| "Uses the current MLBB Items passive.".into())
    }

    pub(crate) fn description(self, mythical: bool) -> Option<String> {
        let scale = if mythical { 1.25 } else { 1. };
        let text = match self {
            Self::Default => return None,
            Self::Clock { interval, hp, magic, stacks, bonus } => format!(
                "Every {} equipped, gain {} HP and {} <#a974ffff>Magic Power<> per stack, up to {} stacks. At full stacks, gain {} Magic Power.",
                seconds(interval), amount(f64::from(hp) * scale), amount(f64::from(magic) * scale),
                amount(stacks), percent(bonus),
            ),
            Self::Thunder { window, damage, hp_percent, slow, max_slow, slow_scale, duration, cooldown } => format!(
                "Within {} of casting, the next basic hit deals {} + {} equipped item HP as <#ffffffff>true damage<> to nearby enemies. Slows for {} by {} plus {} per {} equipped item HP, up to {}. Cooldown: {}.",
                seconds(window), amount(f64::from(damage) * scale), percent(hp_percent * scale),
                seconds(duration), percent(slow), percent(slow_scale), amount(100), percent(max_slow), seconds(cooldown),
            ),
            Self::Defiance { threshold, reduction, hp_percent, seconds: duration, cooldown } => format!(
                "After an enemy <#d94c49ff>physical hit<> above {}, reduce physical damage by {} and deal {} of equipped item HP as <#ffffffff>true damage<> to nearby enemies each second for {}. Cooldown: {}.",
                amount(threshold), percent(reduction), percent(hp_percent * scale),
                seconds(duration), seconds(cooldown),
            ),
            Self::Twilight { threshold, scale } => format!(
                "Reduce enemy damage above {} by {} per {} maximum HP. No cooldown.",
                amount(threshold), percent(scale), amount(100),
            ),
            Self::TwilightOnHit { interval, damage, hp_percent } => format!(
                "Every {}, the next basic hit deals {} + {} of equipped item HP as <#a974ffff>magic damage<>.",
                seconds(interval), amount(f64::from(damage) * scale), percent(hp_percent * scale),
            ),
        };
        Some(format!("<#ff7a3eff>{}<>\n{text}", self.label()))
    }

    pub(crate) fn fields(self) -> Vec<(&'static str, f64)> {
        match self {
            Self::Default => vec![],
            Self::Clock {
                interval,
                hp,
                magic,
                stacks,
                bonus,
            } => vec![
                ("Seconds per stack", interval),
                ("HP per stack", hp.into()),
                ("Magic Power per stack", magic.into()),
                ("Maximum stacks", stacks.into()),
                ("Full-stack Magic Power %", bonus.into()),
            ],
            Self::Thunder {
                window,
                damage,
                hp_percent,
                slow,
                max_slow,
                slow_scale,
                duration,
                cooldown,
            } => vec![
                ("Cast window (s)", window),
                ("Base true damage", damage.into()),
                ("Bonus HP damage %", hp_percent),
                ("Base slow %", slow.into()),
                ("Maximum slow %", max_slow.into()),
                ("Slow % per 100 bonus HP", slow_scale),
                ("Slow duration (s)", duration),
                ("Cooldown (s)", cooldown),
            ],
            Self::Defiance {
                threshold,
                reduction,
                hp_percent,
                seconds,
                cooldown,
            } => vec![
                ("Physical damage threshold", threshold.into()),
                ("Physical reduction %", reduction.into()),
                ("Bonus HP damage % per second", hp_percent),
                ("Duration (s)", seconds.into()),
                ("Cooldown (s)", cooldown),
            ],
            Self::Twilight { threshold, scale } => vec![
                ("Damage threshold", threshold.into()),
                ("Reduction % per 100 max HP", scale),
            ],
            Self::TwilightOnHit {
                interval,
                damage,
                hp_percent,
            } => vec![
                ("Cooldown (s)", interval),
                ("Base magic damage", damage.into()),
                ("Bonus HP damage %", hp_percent),
            ],
        }
    }

    pub(crate) fn with_values(self, values: &[f64]) -> Option<Self> {
        fn in_range(n: f64, low: f64, high: f64) -> bool {
            (low..=high).contains(&n)
        }
        fn whole(n: f64, low: f64, high: f64) -> bool {
            in_range(n, low, high) && n.fract() == 0.
        }
        let time = |n| in_range(n, 0.1, 600.);
        let amount = |n| whole(n, 0., 5000.);
        let percent = |n| in_range(n, 0., 100.);
        Some(match (self, values) {
            (Self::Default, []) => Self::Default,
            (Self::Clock { .. }, &[interval, hp, magic, stacks, bonus])
                if time(interval)
                    && amount(hp)
                    && whole(magic, 0., 500.)
                    && whole(stacks, 1., 30.)
                    && whole(bonus, 0., 100.) =>
            {
                Self::Clock {
                    interval,
                    hp: hp as u16,
                    magic: magic as u16,
                    stacks: stacks as u8,
                    bonus: bonus as u8,
                }
            }
            (
                Self::Thunder { .. },
                &[window, damage, hp_percent, slow, max_slow, slow_scale, duration, cooldown],
            ) if time(window)
                && amount(damage)
                && percent(hp_percent)
                && whole(slow, 0., 100.)
                && whole(max_slow, slow, 100.)
                && percent(slow_scale)
                && time(duration)
                && time(cooldown) =>
            {
                Self::Thunder {
                    window,
                    damage: damage as u16,
                    hp_percent,
                    slow: slow as u8,
                    max_slow: max_slow as u8,
                    slow_scale,
                    duration,
                    cooldown,
                }
            }
            (Self::Defiance { .. }, &[threshold, reduction, hp_percent, seconds, cooldown])
                if amount(threshold)
                    && whole(reduction, 0., 100.)
                    && percent(hp_percent)
                    && whole(seconds, 1., 30.)
                    && time(cooldown) =>
            {
                Self::Defiance {
                    threshold: threshold as u16,
                    reduction: reduction as u8,
                    hp_percent,
                    seconds: seconds as u8,
                    cooldown,
                }
            }
            (Self::Twilight { .. }, &[threshold, scale]) if amount(threshold) && percent(scale) => {
                Self::Twilight {
                    threshold: threshold as u16,
                    scale,
                }
            }
            (Self::TwilightOnHit { .. }, &[interval, damage, hp_percent])
                if time(interval) && amount(damage) && percent(hp_percent) =>
            {
                Self::TwilightOnHit {
                    interval,
                    damage: damage as u16,
                    hp_percent,
                }
            }
            _ => return None,
        })
    }

    pub(crate) fn valid_for(self, key: &str) -> bool {
        let family = match self {
            Self::Default => return Self::supported(key),
            Self::Clock { .. } => "clock_of_destiny",
            Self::Thunder { .. } => "thunder_belt",
            _ => "twilight_armor",
        };
        key == family
            && self
                .with_values(
                    &self
                        .fields()
                        .into_iter()
                        .map(|(_, value)| value)
                        .collect::<Vec<_>>(),
                )
                .is_some()
    }
}

fn amount(value: impl std::fmt::Display) -> String {
    format!("<#e8a800ff>{value}<>")
}

fn seconds(value: impl std::fmt::Display) -> String {
    amount(format_args!("{value}s"))
}

fn percent(value: impl std::fmt::Display) -> String {
    amount(format_args!("{value}%"))
}

static LIVE: RwLock<BTreeMap<String, Preset>> = RwLock::new(BTreeMap::new());

pub(crate) fn sync(passives: &BTreeMap<String, Preset>) {
    if let Ok(mut live) = LIVE.write() {
        *live = passives
            .iter()
            .filter(|(key, preset)| **preset != Preset::Default && preset.valid_for(key))
            .map(|(key, preset)| (key.clone(), *preset))
            .collect();
    }
}

pub(crate) fn any_active() -> bool {
    LIVE.read().is_ok_and(|presets| !presets.is_empty())
}

pub(crate) fn equipped(key: &str) -> Preset {
    if !Preset::supported(key) {
        return Preset::Default;
    }
    LIVE.read()
        .ok()
        .and_then(|presets| presets.get(key).copied())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_round_trip_and_reject_invalid_ranges() {
        for key in ["clock_of_destiny", "thunder_belt", "twilight_armor"] {
            let mut preset = Preset::Default;
            loop {
                preset = preset.next(key);
                assert!(preset.valid_for(key));
                let serialized = serde_json::to_string(&preset).unwrap();
                assert_eq!(serde_json::from_str::<Preset>(&serialized).unwrap(), preset);
                if preset == Preset::Default {
                    break;
                }
                let mut values: Vec<_> = preset
                    .fields()
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect();
                values[0] = f64::NAN;
                assert!(preset.with_values(&values).is_none());
            }
        }
        let thunder = Preset::Default.next("thunder_belt");
        assert!(!thunder.valid_for("twilight_armor"));
    }
}
