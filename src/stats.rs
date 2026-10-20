use bevy::prelude::Component;
use std::ops::{Add, AddAssign};

use serde::Deserialize;

const MIN_ATTACK_SPEED: f32 = 0.05;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct StatModifier {
    pub flat: f32,
    pub increased: f32,
}

impl StatModifier {
    pub fn apply(self, base: f32) -> f32 {
        ((base + self.flat) * (1.0 + self.increased)).max(0.0)
    }
}

impl Add for StatModifier {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            flat: self.flat + other.flat,
            increased: self.increased + other.increased,
        }
    }
}

impl AddAssign for StatModifier {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct StatModifiers {
    pub max_health: StatModifier,
    pub damage: StatModifier,
    pub defense: StatModifier,
    pub move_speed: StatModifier,
    pub attack_speed: StatModifier,
}

impl Add for StatModifiers {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            max_health: self.max_health + other.max_health,
            damage: self.damage + other.damage,
            defense: self.defense + other.defense,
            move_speed: self.move_speed + other.move_speed,
            attack_speed: self.attack_speed + other.attack_speed,
        }
    }
}

impl AddAssign for StatModifiers {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BaseStats {
    pub max_health: f32,
    pub damage: f32,
    pub defense: f32,
    pub move_speed: f32,
    pub attack_speed: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ActorStats {
    base: BaseStats,
    modifiers: StatModifiers,
}

impl ActorStats {
    pub fn new(base: BaseStats) -> Self {
        Self {
            base,
            modifiers: StatModifiers::default(),
        }
    }

    pub fn with_modifiers(mut self, modifiers: StatModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub fn set_modifiers(&mut self, modifiers: StatModifiers) {
        self.modifiers = modifiers;
    }

    pub fn max_health(self) -> f32 {
        self.modifiers.max_health.apply(self.base.max_health)
    }

    pub fn damage(self) -> f32 {
        self.modifiers.damage.apply(self.base.damage)
    }

    pub fn defense(self) -> f32 {
        self.modifiers.defense.apply(self.base.defense)
    }

    pub fn move_speed(self) -> f32 {
        self.modifiers.move_speed.apply(self.base.move_speed)
    }

    pub fn attack_speed(self) -> f32 {
        self.modifiers
            .attack_speed
            .apply(self.base.attack_speed)
            .max(MIN_ATTACK_SPEED)
    }
}
