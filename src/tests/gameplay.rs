use bevy::prelude::Vec2;

use crate::{
    gameplay::{
        Ability, AbilitySpec, ArenaBounds, AttackWindup, Damage, DamageAbility, EncounterPhase,
        EncounterSnapshot, Health, nearest_target, resolve_encounter_phase, step_away, step_toward,
    },
    stats::{ActorStats, BaseStats},
};

#[test]
fn health_clamps_damage_and_healing() {
    let mut health = Health::new(100.0);
    let result = health.apply_damage(
        Damage::new(DamageAbility::HeroAbility("Strike".to_string()), 140.0),
        0.0,
    );
    assert_eq!(health.current(), 0.0);
    assert!(!health.is_alive());
    assert_eq!(result.applied, 100.0);
    assert_eq!(result.health_before, 100.0);
    assert_eq!(result.health_after, 0.0);
    assert!(result.killed);

    health.heal(150.0);
    assert_eq!(health.current(), 100.0);
    assert!(health.is_alive());
}

#[test]
fn negative_health_changes_are_ignored() {
    let mut health = Health::new(100.0);
    let result = health.apply_damage(Damage::new(DamageAbility::EnemyMelee, -10.0), 0.0);
    health.heal(-10.0);
    assert_eq!(health.current(), 100.0);
    assert_eq!(result.applied, 0.0);
    assert!(!result.killed);
}

#[test]
fn changing_max_health_preserves_health_percentage() {
    let mut health = Health::new(100.0);
    health.apply_damage(Damage::new(DamageAbility::EnemyMelee, 25.0), 0.0);
    health.set_max_preserving_percentage(200.0);

    assert_eq!(health.current(), 150.0);
    assert_eq!(health.max(), 200.0);
}

#[test]
fn an_already_defeated_target_is_not_killed_twice() {
    let mut health = Health::new(10.0);
    let first = health.apply_damage(
        Damage::new(DamageAbility::HeroAbility("Whirlwind".to_string()), 10.0),
        0.0,
    );
    let second = health.apply_damage(
        Damage::new(DamageAbility::HeroAbility("Strike".to_string()), 10.0),
        0.0,
    );

    assert!(first.killed);
    assert!(!second.killed);
    assert_eq!(second.applied, 0.0);
}

#[test]
fn ability_cannot_activate_until_its_scaled_cooldown_finishes() {
    let mut ability = Ability::new(AbilitySpec {
        base_damage: 10.0,
        damage_scaling: 1.0,
        range: 2.0,
        cooldown: 1.0,
    });

    assert!(ability.activate(2.0));
    assert!(!ability.activate(2.0));
    ability.tick(0.25);
    assert!(!ability.is_ready());
    ability.tick(0.25);
    assert!(ability.activate(2.0));
}

#[test]
fn attack_windups_complete_and_can_be_cancelled() {
    let mut windup = AttackWindup::default();
    assert!(!windup.is_active());
    assert!(!windup.cancel());

    windup.start(0.5);
    windup.tick(0.2);
    assert!(windup.is_active());
    assert!(!windup.is_complete());
    assert!((windup.remaining() - 0.3).abs() < f32::EPSILON);
    assert!(windup.cancel());
    assert!(!windup.is_active());

    windup.start(0.25);
    windup.tick(1.0);
    assert!(windup.is_complete());
    assert_eq!(windup.remaining(), 0.0);
}

#[test]
fn abilities_scale_damage_from_actor_stats() {
    let ability = Ability::new(AbilitySpec {
        base_damage: 5.0,
        damage_scaling: 1.5,
        range: 2.0,
        cooldown: 1.0,
    });
    let actor = ActorStats::new(BaseStats {
        max_health: 100.0,
        damage: 10.0,
        defense: 0.0,
        move_speed: 4.0,
        attack_speed: 1.0,
    });

    assert_eq!(ability.damage(actor), 20.0);
}

#[test]
fn defense_mitigates_damage_and_reports_the_result() {
    let mut health = Health::new(100.0);
    let result = health.apply_damage(Damage::new(DamageAbility::EnemyMelee, 100.0), 100.0);

    assert_eq!(result.applied, 50.0);
    assert_eq!(result.mitigated, 50.0);
    assert_eq!(result.health_after, 50.0);
}

#[test]
fn nearest_target_returns_distance_and_position() {
    let target = nearest_target(
        Vec2::ZERO,
        [("far", Vec2::new(5.0, 0.0)), ("near", Vec2::new(2.0, 0.0))],
    )
    .expect("a target should be selected");

    assert_eq!(target.id, "near");
    assert_eq!(target.position, Vec2::new(2.0, 0.0));
    assert_eq!(target.distance, 2.0);
}

#[test]
fn movement_stops_at_the_requested_distance_without_overshooting() {
    let movement = step_toward(
        Vec2::ZERO,
        Vec2::new(2.0, 0.0),
        20.0,
        0.5,
        1.0,
        ArenaBounds::square(10.0),
    );

    assert_eq!(movement.position, Vec2::new(1.5, 0.0));
    assert!(movement.reached_stopping_distance);
}

#[test]
fn diagonal_movement_finishes_inside_range_tolerance() {
    let target_position = Vec2::new(4.7, 2.3);
    let movement = step_toward(
        Vec2::ZERO,
        target_position,
        100.0,
        0.8,
        1.0,
        ArenaBounds::square(10.0),
    );
    let target = nearest_target(movement.position, [("loot", target_position)])
        .expect("loot should remain targetable");

    assert!(movement.reached_stopping_distance);
    assert!(target.is_within(0.8));
}

#[test]
fn movement_is_clamped_to_arena_bounds() {
    let bounds = ArenaBounds::square(2.0);
    let toward = step_toward(
        Vec2::new(1.5, 0.0),
        Vec2::new(10.0, 0.0),
        10.0,
        0.0,
        1.0,
        bounds,
    );
    let away = step_away(Vec2::new(-1.5, 0.0), Vec2::ZERO, 10.0, 1.0, bounds);

    assert_eq!(toward.position, Vec2::new(2.0, 0.0));
    assert_eq!(away.position, Vec2::new(-2.0, 0.0));
}

#[test]
fn encounter_phase_is_derived_from_world_state() {
    let snapshot = |hero_alive, living_enemies, loot_remaining| EncounterSnapshot {
        hero_alive,
        living_enemies,
        loot_remaining,
    };

    assert_eq!(
        resolve_encounter_phase(snapshot(true, 3, 0)),
        EncounterPhase::Combat
    );
    assert_eq!(
        resolve_encounter_phase(snapshot(true, 0, 2)),
        EncounterPhase::Looting
    );
    assert_eq!(
        resolve_encounter_phase(snapshot(true, 0, 0)),
        EncounterPhase::Victory
    );
    assert_eq!(
        resolve_encounter_phase(snapshot(false, 3, 2)),
        EncounterPhase::Defeat
    );
}
