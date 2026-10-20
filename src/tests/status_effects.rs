use bevy::prelude::World;

use crate::{
    stats::{StatModifier, StatModifiers},
    status_effects::{StatusEffectKind, StatusEffectSpec, StatusEffects},
};

fn source_entity() -> bevy::prelude::Entity {
    World::new().spawn_empty().id()
}

#[test]
fn periodic_effects_apply_only_for_their_active_time() {
    let source = source_entity();
    let mut effects = StatusEffects::default();
    effects.apply(
        source,
        StatusEffectSpec {
            id: "poison".to_string(),
            duration: 1.0,
            kind: StatusEffectKind::Poison {
                damage_per_second: 4.0,
            },
        },
    );
    effects.apply(
        source,
        StatusEffectSpec::regeneration("regeneration", 0.5, 6.0),
    );

    let first = effects.tick(0.25);
    assert_eq!(first.damage[0].amount, 1.0);
    assert_eq!(first.healing, 1.5);

    let second = effects.tick(0.5);
    assert_eq!(second.damage[0].amount, 2.0);
    assert_eq!(second.healing, 1.5);

    let final_tick = effects.tick(0.5);
    assert_eq!(final_tick.damage[0].amount, 1.0);
    assert_eq!(final_tick.healing, 0.0);
}

#[test]
fn stun_and_temporary_modifiers_expire_together() {
    let source = source_entity();
    let mut effects = StatusEffects::default();
    effects.apply(source, StatusEffectSpec::stun("stun", 1.0));
    effects.apply(
        source,
        StatusEffectSpec::stat_modifier(
            "slow",
            1.0,
            StatModifiers {
                move_speed: StatModifier {
                    increased: -0.25,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
    );

    let active = effects.tick(0.25);
    assert!(active.stunned);
    assert_eq!(active.modifiers.move_speed.increased, -0.25);

    let expired = effects.tick(0.75);
    assert!(!expired.stunned);
    assert_eq!(expired.modifiers, StatModifiers::default());
}

#[test]
fn reapplying_an_effect_refreshes_instead_of_stacking_it() {
    let source = source_entity();
    let mut effects = StatusEffects::default();
    effects.apply(source, StatusEffectSpec::stun("stun", 1.0));
    effects.tick(0.75);
    effects.apply(source, StatusEffectSpec::stun("stun", 1.0));

    assert!(effects.tick(0.75).stunned);
    assert!(!effects.tick(0.25).stunned);
}

#[test]
fn invalid_effect_values_are_rejected() {
    let invalid = StatusEffectSpec {
        id: "bad_poison".to_string(),
        duration: -1.0,
        kind: StatusEffectKind::Poison {
            damage_per_second: f32::NAN,
        },
    };

    assert!(invalid.validate().is_err());
}
