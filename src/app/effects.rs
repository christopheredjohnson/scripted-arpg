use bevy::prelude::*;

use super::model::{DealDamage, GameSet};
use crate::{
    gameplay::{Damage, DamageAbility, Health},
    stats::ActorStats,
    status_effects::{ApplyStatusEffect, StatusEffects},
};

pub(super) struct StatusEffectPlugin;

impl Plugin for StatusEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ApplyStatusEffect>().add_systems(
            Update,
            (apply_status_effects, tick_status_effects)
                .chain()
                .in_set(GameSet::Effects),
        );
    }
}

fn apply_status_effects(
    mut requests: MessageReader<ApplyStatusEffect>,
    mut targets: Query<&mut StatusEffects>,
) {
    for request in requests.read() {
        if let Ok(mut effects) = targets.get_mut(request.target) {
            effects.apply(request.source, request.effect.clone());
        }
    }
}

fn tick_status_effects(
    time: Res<Time>,
    mut actors: Query<(Entity, &mut StatusEffects, &mut ActorStats, &mut Health)>,
    mut damage_requests: MessageWriter<DealDamage>,
) {
    for (entity, mut effects, mut actor_stats, mut health) in &mut actors {
        let tick = effects.tick(time.delta_secs());
        let old_max_health = health.max();
        let modifiers_changed = actor_stats.set_temporary_modifiers(tick.modifiers);
        let new_max_health = actor_stats.max_health();
        if modifiers_changed && (new_max_health - old_max_health).abs() > f32::EPSILON {
            health.set_max_preserving_percentage(new_max_health);
        }
        health.heal(tick.healing);

        for periodic_damage in tick.damage {
            if periodic_damage.amount > 0.0 {
                damage_requests.write(DealDamage {
                    source: periodic_damage.source,
                    target: entity,
                    damage: Damage::new(DamageAbility::Poison, periodic_damage.amount),
                });
            }
        }
    }
}
