use bevy::prelude::{Component, Entity, Message};
use serde::Deserialize;

use crate::stats::StatModifiers;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StatusEffectSpec {
    pub id: String,
    pub duration: f32,
    pub kind: StatusEffectKind,
}

impl StatusEffectSpec {
    pub fn regeneration(id: impl Into<String>, duration: f32, healing_per_second: f32) -> Self {
        Self {
            id: id.into(),
            duration,
            kind: StatusEffectKind::Regeneration { healing_per_second },
        }
    }

    #[cfg(test)]
    pub fn stun(id: impl Into<String>, duration: f32) -> Self {
        Self {
            id: id.into(),
            duration,
            kind: StatusEffectKind::Stun,
        }
    }

    pub fn stat_modifier(id: impl Into<String>, duration: f32, modifiers: StatModifiers) -> Self {
        Self {
            id: id.into(),
            duration,
            kind: StatusEffectKind::StatModifier { modifiers },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("status effect id cannot be empty".to_string());
        }
        if !self.duration.is_finite() || self.duration <= 0.0 {
            return Err(format!(
                "status effect `{}` must have a finite positive duration",
                self.id
            ));
        }

        match self.kind {
            StatusEffectKind::Poison { damage_per_second } => {
                validate_positive_rate(&self.id, "damage_per_second", damage_per_second)
            }
            StatusEffectKind::Regeneration { healing_per_second } => {
                validate_positive_rate(&self.id, "healing_per_second", healing_per_second)
            }
            StatusEffectKind::Stun => Ok(()),
            StatusEffectKind::StatModifier { modifiers } => {
                let values = [
                    modifiers.max_health.flat,
                    modifiers.max_health.increased,
                    modifiers.damage.flat,
                    modifiers.damage.increased,
                    modifiers.defense.flat,
                    modifiers.defense.increased,
                    modifiers.move_speed.flat,
                    modifiers.move_speed.increased,
                    modifiers.attack_speed.flat,
                    modifiers.attack_speed.increased,
                ];
                if values.into_iter().any(|value| !value.is_finite())
                    || modifiers.max_health.flat < 0.0
                    || modifiers.max_health.increased <= -1.0
                {
                    return Err(format!(
                        "status effect `{}` has invalid stat modifiers",
                        self.id
                    ));
                }
                Ok(())
            }
        }
    }

    pub fn label(&self) -> &str {
        match self.kind {
            StatusEffectKind::Poison { .. } => "Poison",
            StatusEffectKind::Regeneration { .. } => "Regeneration",
            StatusEffectKind::Stun => "Stun",
            StatusEffectKind::StatModifier { .. } => "Stat modifier",
        }
    }
}

fn validate_positive_rate(id: &str, name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!(
            "status effect `{id}` must have a finite positive {name}"
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum StatusEffectKind {
    Poison { damage_per_second: f32 },
    Regeneration { healing_per_second: f32 },
    Stun,
    StatModifier { modifiers: StatModifiers },
}

#[derive(Clone, Debug)]
struct ActiveStatusEffect {
    source: Entity,
    spec: StatusEffectSpec,
    remaining: f32,
}

#[derive(Component, Debug, Default)]
pub struct StatusEffects {
    active: Vec<ActiveStatusEffect>,
    stunned: bool,
}

impl StatusEffects {
    pub fn apply(&mut self, source: Entity, spec: StatusEffectSpec) {
        if let Some(active) = self
            .active
            .iter_mut()
            .find(|active| active.spec.id == spec.id)
        {
            active.source = source;
            active.remaining = spec.duration;
            active.spec = spec;
        } else {
            let remaining = spec.duration;
            self.active.push(ActiveStatusEffect {
                source,
                spec,
                remaining,
            });
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) -> StatusEffectTick {
        let delta_seconds = delta_seconds.max(0.0);
        let mut tick = StatusEffectTick::default();

        for active in &mut self.active {
            let active_seconds = delta_seconds.min(active.remaining.max(0.0));
            match active.spec.kind {
                StatusEffectKind::Poison { damage_per_second } => {
                    tick.damage.push(PeriodicDamage {
                        source: active.source,
                        amount: damage_per_second * active_seconds,
                    });
                }
                StatusEffectKind::Regeneration { healing_per_second } => {
                    tick.healing += healing_per_second * active_seconds;
                }
                StatusEffectKind::Stun | StatusEffectKind::StatModifier { .. } => {}
            }
            active.remaining -= delta_seconds;
        }
        self.active.retain(|active| active.remaining > 0.0);

        self.stunned = self
            .active
            .iter()
            .any(|active| matches!(active.spec.kind, StatusEffectKind::Stun));
        for active in &self.active {
            if let StatusEffectKind::StatModifier { modifiers } = active.spec.kind {
                tick.modifiers += modifiers;
            }
        }
        tick.stunned = self.stunned;
        tick
    }

    pub fn is_stunned(&self) -> bool {
        self.stunned
    }

    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.active.iter().map(|active| active.spec.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeriodicDamage {
    pub source: Entity,
    pub amount: f32,
}

#[derive(Debug, Default, PartialEq)]
pub struct StatusEffectTick {
    pub damage: Vec<PeriodicDamage>,
    pub healing: f32,
    pub modifiers: StatModifiers,
    pub stunned: bool,
}

#[derive(Message, Clone, Debug)]
pub struct ApplyStatusEffect {
    pub source: Entity,
    pub target: Entity,
    pub effect: StatusEffectSpec,
}
