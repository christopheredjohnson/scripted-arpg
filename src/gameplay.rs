use bevy::prelude::{Component, Resource, Vec2};

use crate::stats::{ActorStats, BaseStats};

pub const DECISION_INTERVAL: f32 = 0.2;
const DISTANCE_EPSILON: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub struct ArenaBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl ArenaBounds {
    pub fn square(half_extent: f32) -> Self {
        let half_extent = half_extent.max(0.0);
        Self {
            min: Vec2::splat(-half_extent),
            max: Vec2::splat(half_extent),
        }
    }

    pub fn clamp(self, position: Vec2) -> Vec2 {
        position.clamp(self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target<T> {
    pub id: T,
    pub position: Vec2,
    pub distance: f32,
}

impl<T> Target<T> {
    pub fn is_within(&self, range: f32) -> bool {
        within_range(self.distance, range)
    }
}

pub fn within_range(distance: f32, range: f32) -> bool {
    distance <= range.max(0.0) + DISTANCE_EPSILON
}

pub fn nearest_target<T>(
    origin: Vec2,
    candidates: impl IntoIterator<Item = (T, Vec2)>,
) -> Option<Target<T>> {
    candidates
        .into_iter()
        .map(|(id, position)| Target {
            id,
            position,
            distance: origin.distance(position),
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementStep {
    pub position: Vec2,
    pub direction: Vec2,
    pub reached_stopping_distance: bool,
}

pub fn step_toward(
    current: Vec2,
    target: Vec2,
    speed: f32,
    stopping_distance: f32,
    delta_seconds: f32,
    bounds: ArenaBounds,
) -> MovementStep {
    let offset = target - current;
    let distance = offset.length();
    let direction = offset.normalize_or_zero();
    let distance_to_travel = (distance - stopping_distance.max(0.0)).max(0.0);
    let travel = (speed.max(0.0) * delta_seconds.max(0.0)).min(distance_to_travel);
    let position = bounds.clamp(current + direction * travel);

    MovementStep {
        position,
        direction: (position - current).normalize_or_zero(),
        reached_stopping_distance: within_range(position.distance(target), stopping_distance),
    }
}

pub fn step_away(
    current: Vec2,
    threat: Vec2,
    speed: f32,
    delta_seconds: f32,
    bounds: ArenaBounds,
) -> MovementStep {
    let direction = (current - threat).normalize_or_zero();
    let position = bounds.clamp(current + direction * speed.max(0.0) * delta_seconds.max(0.0));

    MovementStep {
        position,
        direction: (position - current).normalize_or_zero(),
        reached_stopping_distance: false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DamageAbility {
    HeroAbility(String),
    EnemyMelee,
    Poison,
}

impl DamageAbility {
    pub fn label(&self) -> &str {
        match self {
            Self::HeroAbility(name) => name,
            Self::EnemyMelee => "Enemy melee",
            Self::Poison => "Poison",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Damage {
    pub ability: DamageAbility,
    pub amount: f32,
}

impl Damage {
    pub fn new(ability: DamageAbility, amount: f32) -> Self {
        Self {
            ability,
            amount: amount.max(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageResult {
    pub damage: Damage,
    pub applied: f32,
    pub mitigated: f32,
    pub health_before: f32,
    pub health_after: f32,
    pub killed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AbilitySpec {
    pub base_damage: f32,
    pub damage_scaling: f32,
    pub range: f32,
    pub cooldown: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct HeroBalance {
    pub stats: ActorStats,
    pub loot_move_speed_multiplier: f32,
    pub retreat_move_speed_multiplier: f32,
    pub retreat_healing_per_second: f32,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct GameBalance {
    pub hero: HeroBalance,
    pub arena_bounds: ArenaBounds,
    pub loot_pickup_range: f32,
    pub loot_detection_range: f32,
}

impl Default for GameBalance {
    fn default() -> Self {
        Self {
            hero: HeroBalance {
                stats: ActorStats::new(BaseStats {
                    max_health: 120.0,
                    damage: 10.0,
                    defense: 0.0,
                    move_speed: 4.6,
                    attack_speed: 1.0,
                })
                .with_modifiers(Default::default()),
                loot_move_speed_multiplier: 5.2 / 4.6,
                retreat_move_speed_multiplier: 5.5 / 4.6,
                retreat_healing_per_second: 18.0,
            },
            arena_bounds: ArenaBounds::square(9.5),
            loot_pickup_range: 0.8,
            loot_detection_range: 14.0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Health {
    current: f32,
    max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        assert!(max > 0.0, "maximum health must be positive");
        Self { current: max, max }
    }

    pub fn current(self) -> f32 {
        self.current
    }

    pub fn max(self) -> f32 {
        self.max
    }

    pub fn percent(self) -> f32 {
        self.current / self.max
    }

    pub fn is_alive(self) -> bool {
        self.current > 0.0
    }

    pub fn apply_damage(&mut self, damage: Damage, defense: f32) -> DamageResult {
        let health_before = self.current;
        let damage_amount = damage.amount;
        let damage_after_defense = damage.amount * 100.0 / (100.0 + defense.max(0.0));
        self.current = (self.current - damage_after_defense).max(0.0);

        DamageResult {
            damage,
            applied: health_before - self.current,
            mitigated: damage_amount - damage_after_defense,
            health_before,
            health_after: self.current,
            killed: health_before > 0.0 && self.current <= 0.0,
        }
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount.max(0.0)).min(self.max);
    }

    pub fn set_max_preserving_percentage(&mut self, max: f32) {
        assert!(max > 0.0, "maximum health must be positive");
        let percentage = self.percent();
        self.max = max;
        self.current = self.max * percentage;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Cooldown {
    remaining: f32,
}

impl Cooldown {
    pub fn is_ready(self) -> bool {
        self.remaining <= 0.0
    }

    pub fn remaining(self) -> f32 {
        self.remaining
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        self.remaining = (self.remaining - delta_seconds.max(0.0)).max(0.0);
    }

    pub fn start(&mut self, duration: f32) {
        self.remaining = duration.max(0.0);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackWindup {
    remaining: Option<f32>,
}

impl AttackWindup {
    pub fn start(&mut self, duration: f32) {
        self.remaining = Some(duration.max(0.0));
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        if let Some(remaining) = &mut self.remaining {
            *remaining = (*remaining - delta_seconds.max(0.0)).max(0.0);
        }
    }

    pub fn is_active(self) -> bool {
        self.remaining.is_some()
    }

    pub fn is_complete(self) -> bool {
        self.remaining == Some(0.0)
    }

    pub fn remaining(self) -> f32 {
        self.remaining.unwrap_or(0.0)
    }

    pub fn cancel(&mut self) -> bool {
        self.remaining.take().is_some()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ability {
    spec: AbilitySpec,
    cooldown: Cooldown,
}

impl Ability {
    pub fn new(spec: AbilitySpec) -> Self {
        Self {
            spec,
            cooldown: Cooldown::default(),
        }
    }

    pub fn damage(self, actor: ActorStats) -> f32 {
        self.spec.base_damage + actor.damage() * self.spec.damage_scaling.max(0.0)
    }

    pub fn range(self) -> f32 {
        self.spec.range
    }

    pub fn is_ready(self) -> bool {
        self.cooldown.is_ready()
    }

    pub fn remaining_cooldown(self) -> f32 {
        self.cooldown.remaining()
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        self.cooldown.tick(delta_seconds);
    }

    pub fn activate(&mut self, attack_speed: f32) -> bool {
        if !self.is_ready() {
            return false;
        }
        self.cooldown
            .start(self.spec.cooldown / attack_speed.max(0.05));
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterPhase {
    Combat,
    Looting,
    Victory,
    Defeat,
}

#[derive(Clone, Copy, Debug)]
pub struct EncounterSnapshot {
    pub hero_alive: bool,
    pub living_enemies: usize,
    pub loot_remaining: usize,
}

pub fn resolve_encounter_phase(snapshot: EncounterSnapshot) -> EncounterPhase {
    if !snapshot.hero_alive {
        EncounterPhase::Defeat
    } else if snapshot.living_enemies > 0 {
        EncounterPhase::Combat
    } else if snapshot.loot_remaining > 0 {
        EncounterPhase::Looting
    } else {
        EncounterPhase::Victory
    }
}
