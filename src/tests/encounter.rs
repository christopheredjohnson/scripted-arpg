use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use crate::{
    app::{
        ArenaVisuals, CombatLog, Enemy, GameStats, GameplayPlugin, Hero, HeroBrain, HeroCombat,
        Loot, ProgressionNotice,
    },
    content::ContentCatalog,
    gameplay::{AttackWindup, Cooldown, EncounterPhase, GameBalance, Health},
    items::{Equipment, EquipmentSlot, Inventory},
    progression::PlayerProgression,
    scripting::{HeroAction, ScriptRuntime},
    stats::ActorStats,
    status_effects::{StatusEffectSpec, StatusEffects},
};

#[test]
fn enemy_melee_damage_lands_only_after_its_windup() {
    let (mut app, hero_entity, enemy_entity) = windup_test_app();
    let full_health = app
        .world()
        .get::<Health>(hero_entity)
        .expect("hero should exist")
        .current();

    app.update();
    assert_eq!(
        app.world()
            .get::<Health>(hero_entity)
            .expect("hero should still exist")
            .current(),
        full_health
    );
    assert!(
        app.world()
            .get::<Enemy>(enemy_entity)
            .expect("enemy should exist")
            .attack_windup
            .is_active()
    );

    app.update();
    assert_eq!(
        app.world()
            .get::<Health>(hero_entity)
            .expect("hero should still exist")
            .current(),
        full_health
    );

    app.update();
    assert!(
        app.world()
            .get::<Health>(hero_entity)
            .expect("hero should still exist")
            .current()
            < full_health
    );
}

#[test]
fn stunning_a_winding_enemy_cancels_damage_and_records_an_interrupt() {
    let (mut app, hero_entity, enemy_entity) = windup_test_app();
    app.update();

    {
        let mut effects = app
            .world_mut()
            .get_mut::<StatusEffects>(enemy_entity)
            .expect("enemy effects should exist");
        effects.apply(hero_entity, StatusEffectSpec::stun("test_interrupt", 1.0));
        effects.tick(0.0);
    }
    let health_before = app
        .world()
        .get::<Health>(hero_entity)
        .expect("hero should exist")
        .current();

    app.update();

    assert_eq!(
        app.world()
            .get::<Health>(hero_entity)
            .expect("hero should still exist")
            .current(),
        health_before
    );
    assert!(
        !app.world()
            .get::<Enemy>(enemy_entity)
            .expect("enemy should exist")
            .attack_windup
            .is_active()
    );
    assert_eq!(app.world().resource::<GameStats>().interrupts, 1);
}

#[test]
fn generic_area_abilities_honor_the_content_target_limit() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let balance = GameBalance::default();
    let enemy_archetype = content
        .enemy_archetype("grunt")
        .expect("grunt should exist");
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test("fn decide(world) { #{ action: \"cast\", ability: \"cleave\" } }")
        .expect("Cleave test script should compile");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(balance)
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(PlayerProgression::default())
        .insert_resource(ProgressionNotice(String::new()))
        .insert_resource(ArenaVisuals::default())
        .insert_resource(content.clone())
        .insert_resource(runtime)
        .add_plugins(GameplayPlugin);
    app.world_mut().spawn((
        Hero,
        Transform::default(),
        Health::new(balance.hero.stats.max_health()),
        balance.hero.stats,
        HeroBrain {
            action: HeroAction::Idle,
            decision_cooldown: 0.0,
        },
        HeroCombat::from_profile(&content, &PlayerProgression::default())
            .expect("default Vanguard loadout should resolve"),
        Inventory::default(),
        Equipment::default(),
        StatusEffects::default(),
    ));
    for x in [-1.5, -0.5, 0.5, 1.5] {
        app.world_mut().spawn((
            Enemy {
                attack_cooldown: Cooldown::default(),
                attack_windup: AttackWindup::default(),
                attack_range: enemy_archetype.attack_range,
                attack_cooldown_duration: enemy_archetype.attack_cooldown,
                attack_windup_duration: enemy_archetype.attack_windup,
                drop_table: enemy_archetype.drop_table.clone(),
                xp_reward: enemy_archetype.xp_reward,
                on_hit_effect: None,
            },
            Transform::from_xyz(x, 0.0, 0.0),
            Health::new(100.0),
            enemy_archetype.actor_stats_for_wave(content.wave_scaling, 0),
            StatusEffects::default(),
        ));
    }

    app.update();

    let damaged = app
        .world_mut()
        .query_filtered::<&Health, With<Enemy>>()
        .iter(app.world())
        .filter(|health| health.current() < 100.0)
        .count();
    assert_eq!(damaged, 3);
}

#[test]
fn generic_self_cast_abilities_apply_their_content_effects() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let balance = GameBalance::default();
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test("fn decide(world) { #{ action: \"cast\", ability: \"guard\" } }")
        .expect("Guard test script should compile");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.1,
        )))
        .insert_resource(balance)
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(PlayerProgression::default())
        .insert_resource(ProgressionNotice(String::new()))
        .insert_resource(ArenaVisuals::default())
        .insert_resource(content.clone())
        .insert_resource(runtime)
        .add_plugins(GameplayPlugin);
    let hero = app
        .world_mut()
        .spawn((
            Hero,
            Transform::default(),
            Health::new(balance.hero.stats.max_health()),
            balance.hero.stats,
            HeroBrain {
                action: HeroAction::Idle,
                decision_cooldown: 0.0,
            },
            HeroCombat::from_profile(&content, &PlayerProgression::default())
                .expect("default Vanguard loadout should resolve"),
            Inventory::default(),
            Equipment::default(),
            StatusEffects::default(),
        ))
        .id();

    app.update();
    app.update();

    let stats = app
        .world()
        .get::<ActorStats>(hero)
        .expect("hero stats should exist");
    let effects = app
        .world()
        .get::<StatusEffects>(hero)
        .expect("hero effects should exist");
    let combat = app
        .world()
        .get::<HeroCombat>(hero)
        .expect("hero combat should exist");
    assert_eq!(stats.defense(), balance.hero.stats.defense() + 30.0);
    assert_eq!(effects.labels().count(), 2);
    assert!(
        combat
            .ability("guard")
            .expect("Guard should be equipped")
            .remaining_cooldown()
            > 0.0
    );
}

#[test]
fn encounter_damage_death_xp_loot_and_equipment_flow_end_to_end() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let balance = GameBalance::default();
    let enemy_archetype = content
        .enemy_archetype("grunt")
        .expect("grunt should exist");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(balance)
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(PlayerProgression::default())
        .insert_resource(ProgressionNotice(String::new()))
        .insert_resource(ArenaVisuals::default())
        .insert_resource(content.clone())
        .insert_resource(ScriptRuntime::new())
        .add_plugins(GameplayPlugin);

    let hero_entity = app
        .world_mut()
        .spawn((
            Hero,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Health::new(balance.hero.stats.max_health()),
            balance.hero.stats,
            HeroBrain {
                action: HeroAction::Cast("strike".to_string()),
                decision_cooldown: 0.0,
            },
            HeroCombat::from_profile(&content, &PlayerProgression::default())
                .expect("default Vanguard loadout should resolve"),
            Inventory::default(),
            Equipment::default(),
            StatusEffects::default(),
        ))
        .id();
    app.world_mut().spawn((
        Enemy {
            attack_cooldown: Cooldown::default(),
            attack_windup: AttackWindup::default(),
            attack_range: enemy_archetype.attack_range,
            attack_cooldown_duration: enemy_archetype.attack_cooldown,
            attack_windup_duration: enemy_archetype.attack_windup,
            drop_table: enemy_archetype.drop_table.clone(),
            xp_reward: enemy_archetype.xp_reward,
            on_hit_effect: enemy_archetype.on_hit_effect.clone(),
        },
        Transform::from_xyz(0.5, 0.0, 0.0),
        Health::new(1.0),
        enemy_archetype.actor_stats_for_wave(content.wave_scaling, 0),
        StatusEffects::default(),
    ));

    app.update();

    assert_eq!(
        app.world().resource::<PlayerProgression>().experience(),
        enemy_archetype.xp_reward
    );
    assert_eq!(
        app.world().resource::<GameStats>().phase,
        EncounterPhase::Looting
    );
    assert_eq!(entity_count::<Enemy>(app.world_mut()), 0);
    assert_eq!(entity_count::<Loot>(app.world_mut()), 1);
    let health_before_transition = app
        .world()
        .get::<Health>(hero_entity)
        .expect("hero should survive the first encounter")
        .current();

    {
        let mut brain_query = app
            .world_mut()
            .query_filtered::<&mut HeroBrain, With<Hero>>();
        let mut brain = brain_query
            .single_mut(app.world_mut())
            .expect("hero brain should exist");
        brain.action = HeroAction::Loot;
    }
    app.update();

    assert_eq!(entity_count::<Loot>(app.world_mut()), 0);
    assert_eq!(
        app.world().resource::<GameStats>().phase,
        EncounterPhase::Combat
    );
    assert_eq!(app.world().resource::<GameStats>().wave_index, 1);
    assert!(entity_count::<Enemy>(app.world_mut()) > 0);
    assert_eq!(entity_count::<Hero>(app.world_mut()), 1);
    assert_eq!(
        app.world()
            .get::<Health>(hero_entity)
            .expect("wave transition should preserve the hero")
            .current(),
        health_before_transition
    );
    let mut hero_query = app
        .world_mut()
        .query_filtered::<(&Inventory, &Equipment), With<Hero>>();
    let (inventory, equipment) = hero_query
        .single(app.world())
        .expect("hero inventory should exist");
    assert_eq!(inventory.len(), 1);
    assert_eq!(
        equipment.item_name(EquipmentSlot::Boots),
        Some("Scout Boots")
    );

    let profile = app.world().resource::<PlayerProgression>();
    assert_eq!(profile.items()[0].base_id, "scout_boots");
    assert_eq!(profile.items()[0].item_level, 1);
    assert_eq!(
        profile.equipped_instance(EquipmentSlot::Boots),
        Some(profile.items()[0].instance_id.as_str())
    );

    let enemies = app
        .world_mut()
        .query_filtered::<Entity, With<Enemy>>()
        .iter(app.world())
        .collect::<Vec<_>>();
    for enemy in enemies {
        app.world_mut().despawn(enemy);
    }
    {
        let mut stats = app.world_mut().resource_mut::<GameStats>();
        stats.wave_index = 2;
        stats.phase = EncounterPhase::Victory;
    }
    app.update();

    assert_eq!(app.world().resource::<GameStats>().wave_index, 3);
    assert_eq!(entity_count::<Enemy>(app.world_mut()), 8);
}

fn entity_count<T: Component>(world: &mut World) -> usize {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .count()
}

fn windup_test_app() -> (App, Entity, Entity) {
    let content = ContentCatalog::load().expect("shipped content should load");
    let balance = GameBalance::default();
    let enemy_archetype = content
        .enemy_archetype("grunt")
        .expect("grunt should exist");
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test("fn decide(world) { #{ action: \"idle\" } }")
        .expect("idle test script should compile");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.25,
        )))
        .insert_resource(balance)
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(PlayerProgression::default())
        .insert_resource(ProgressionNotice(String::new()))
        .insert_resource(ArenaVisuals::default())
        .insert_resource(content.clone())
        .insert_resource(runtime)
        .add_plugins(GameplayPlugin);

    let hero_entity = app
        .world_mut()
        .spawn((
            Hero,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Health::new(balance.hero.stats.max_health()),
            balance.hero.stats,
            HeroBrain {
                action: HeroAction::Idle,
                decision_cooldown: 0.0,
            },
            HeroCombat::from_profile(&content, &PlayerProgression::default())
                .expect("default Vanguard loadout should resolve"),
            Inventory::default(),
            Equipment::default(),
            StatusEffects::default(),
        ))
        .id();
    let enemy_entity = app
        .world_mut()
        .spawn((
            Enemy {
                attack_cooldown: Cooldown::default(),
                attack_windup: AttackWindup::default(),
                attack_range: enemy_archetype.attack_range,
                attack_cooldown_duration: enemy_archetype.attack_cooldown,
                attack_windup_duration: enemy_archetype.attack_windup,
                drop_table: enemy_archetype.drop_table.clone(),
                xp_reward: enemy_archetype.xp_reward,
                on_hit_effect: enemy_archetype.on_hit_effect.clone(),
            },
            Transform::from_xyz(0.5, 0.0, 0.0),
            Health::new(enemy_archetype.stats.max_health),
            enemy_archetype.actor_stats_for_wave(content.wave_scaling, 0),
            StatusEffects::default(),
        ))
        .id();

    (app, hero_entity, enemy_entity)
}
