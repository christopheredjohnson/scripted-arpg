use bevy::{ecs::system::SystemParam, prelude::*};

use super::model::{
    ArenaEntity, ArenaVisuals, CombatLog, Enemy, GameStats, Hero, HeroBrain, HeroCombat,
    InputStage, Loot, ProgressionNotice, ResolutionStage,
};
use crate::{
    content::ContentCatalog,
    gameplay::{
        AttackWindup, Cooldown, EncounterPhase, EncounterSnapshot, GameBalance, Health,
        resolve_encounter_phase,
    },
    items::{Equipment, EquipmentSlot, Inventory, LootRarity},
    progression::PlayerProgression,
    scripting::HeroAction,
    status_effects::StatusEffects,
};

pub(super) struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, reset_arena.in_set(InputStage::ArenaReset))
            .add_systems(Update, resolve_deaths.in_set(ResolutionStage::Deaths))
            .add_systems(
                Update,
                advance_encounter.in_set(ResolutionStage::WaveAdvance),
            );
    }
}

#[derive(SystemParam)]
struct ArenaResetContext<'w> {
    stats: ResMut<'w, GameStats>,
    combat_log: ResMut<'w, CombatLog>,
    visuals: Res<'w, ArenaVisuals>,
    balance: Res<'w, GameBalance>,
    content: Res<'w, ContentCatalog>,
    progression: Res<'w, PlayerProgression>,
}

#[derive(SystemParam)]
struct DeathContext<'w> {
    visuals: Res<'w, ArenaVisuals>,
    content: Res<'w, ContentCatalog>,
    stats: ResMut<'w, GameStats>,
    progression: ResMut<'w, PlayerProgression>,
    progression_notice: ResMut<'w, ProgressionNotice>,
}

#[derive(SystemParam)]
struct EncounterAdvanceContext<'w> {
    visuals: Res<'w, ArenaVisuals>,
    content: Res<'w, ContentCatalog>,
    stats: ResMut<'w, GameStats>,
    progression_notice: ResMut<'w, ProgressionNotice>,
}

pub(super) fn spawn_encounter(
    commands: &mut Commands,
    visuals: &ArenaVisuals,
    balance: &GameBalance,
    content: &ContentCatalog,
    progression: &PlayerProgression,
) {
    let equipment = equipment_from_profile(progression);
    let inventory = Inventory::from_items(progression.items().to_vec());
    let hero_stats = balance
        .hero
        .stats
        .with_modifiers(equipment.modifiers() + progression.modifiers());
    commands.spawn((
        ArenaEntity,
        Hero,
        Health::new(hero_stats.max_health()),
        hero_stats,
        HeroBrain {
            action: HeroAction::Idle,
            decision_cooldown: 0.0,
        },
        HeroCombat::from_profile(content, progression)
            .expect("validated profile abilities must resolve"),
        inventory,
        equipment,
        StatusEffects::default(),
        Mesh3d(visuals.hero_mesh.clone()),
        MeshMaterial3d(visuals.hero_material.clone()),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));

    spawn_enemy_wave(commands, visuals, content, 0);
}

fn spawn_enemy_wave(
    commands: &mut Commands,
    visuals: &ArenaVisuals,
    content: &ContentCatalog,
    wave_index: usize,
) {
    let encounter = content
        .encounter_for_wave(wave_index)
        .expect("validated content must contain an encounter for every wave");
    for group_id in &encounter.spawn_groups {
        let group = content
            .spawn_group(group_id)
            .expect("validated encounter must reference a known spawn group");
        for (index, spawn) in group.enemies.iter().enumerate() {
            let archetype = content
                .enemy_archetype(&spawn.archetype)
                .expect("validated spawn must reference a known enemy archetype");
            let actor_stats = archetype.actor_stats_for_wave(content.wave_scaling, wave_index);
            let mut attack_cooldown = Cooldown::default();
            attack_cooldown.start(index as f32 * group.attack_stagger);

            commands.spawn((
                ArenaEntity,
                Enemy {
                    attack_cooldown,
                    attack_windup: AttackWindup::default(),
                    attack_range: archetype.attack_range,
                    attack_cooldown_duration: archetype.attack_cooldown,
                    attack_windup_duration: archetype.attack_windup,
                    drop_table: archetype.drop_table.clone(),
                    xp_reward: content
                        .wave_scaling
                        .scale_xp(archetype.xp_reward, wave_index),
                    on_hit_effect: archetype.on_hit_effect.clone(),
                },
                Health::new(actor_stats.max_health()),
                actor_stats,
                StatusEffects::default(),
                Mesh3d(visuals.enemy_mesh.clone()),
                MeshMaterial3d(visuals.enemy_material.clone()),
                Transform::from_xyz(spawn.x, 0.8, spawn.z),
            ));
        }
    }
}

pub(super) fn validate_profile_items(
    progression: &PlayerProgression,
    content: &ContentCatalog,
) -> Result<(), String> {
    progression.validate_items(|item| {
        content.validate_item_instance(item)?;
        Ok(item.slot)
    })
}

pub(super) fn equipment_from_profile(progression: &PlayerProgression) -> Equipment {
    let mut equipment = Equipment::default();
    for slot in [
        EquipmentSlot::Weapon,
        EquipmentSlot::Armor,
        EquipmentSlot::Boots,
    ] {
        if let Some(item) = progression
            .equipped_instance(slot)
            .and_then(|instance_id| progression.item(instance_id))
        {
            equipment.equip(item);
        }
    }
    equipment
}

fn reset_arena(
    mut commands: Commands,
    mut context: ArenaResetContext,
    arena_entities: Query<Entity, With<ArenaEntity>>,
) {
    if !context.stats.reset_requested {
        return;
    }

    for entity in &arena_entities {
        commands.entity(entity).despawn();
    }
    *context.stats = GameStats::default();
    *context.combat_log = CombatLog::default();
    spawn_encounter(
        &mut commands,
        &context.visuals,
        &context.balance,
        &context.content,
        &context.progression,
    );
}

fn resolve_deaths(
    mut commands: Commands,
    mut context: DeathContext,
    hero: Query<&Health, With<Hero>>,
    enemies: Query<(Entity, &Transform, &Health, &Enemy)>,
    loot: Query<Entity, With<Loot>>,
) {
    let Ok(hero_health) = hero.single() else {
        return;
    };
    let mut living_enemies = 0;
    let mut dropped_loot = 0;
    for (entity, transform, health, enemy) in &enemies {
        if health.is_alive() {
            living_enemies += 1;
            continue;
        }

        let item_level = u32::try_from(context.stats.wave_index)
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        let seed = entity.to_bits()
            ^ (context.stats.wave_index as u64).rotate_left(21)
            ^ u64::from(context.stats.kills).rotate_left(42);
        let item = context
            .content
            .generate_drop(&enemy.drop_table, item_level, seed)
            .expect("validated drop table must generate an item");
        let material = match item.rarity {
            LootRarity::Common => context.visuals.common_material.clone(),
            LootRarity::Rare => context.visuals.rare_material.clone(),
            LootRarity::Epic => context.visuals.epic_material.clone(),
        };

        commands.entity(entity).despawn();
        commands.spawn((
            ArenaEntity,
            Loot {
                item,
                base_y: 0.45,
                phase: context.stats.kills as f32 * 0.7,
            },
            Mesh3d(context.visuals.loot_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(transform.translation.x, 0.45, transform.translation.z)
                .with_rotation(Quat::from_rotation_y(0.75)),
        ));
        context.stats.kills += 1;
        context.stats.score += 5;
        let levels_gained = context.progression.award_experience(enemy.xp_reward);
        if levels_gained > 0 {
            context.progression_notice.0 = format!(
                "LEVEL UP · Level {} · {} point{} available",
                context.progression.level(),
                context.progression.unspent_points(),
                if context.progression.unspent_points() == 1 {
                    ""
                } else {
                    "s"
                }
            );
        }
        dropped_loot += 1;
    }

    context.stats.phase = resolve_encounter_phase(EncounterSnapshot {
        hero_alive: hero_health.is_alive(),
        living_enemies,
        loot_remaining: loot.iter().count() + dropped_loot,
    });
}

fn advance_encounter(mut commands: Commands, mut context: EncounterAdvanceContext) {
    if context.stats.phase != EncounterPhase::Victory {
        return;
    }

    let next_index = context.stats.wave_index.saturating_add(1);
    let next_encounter = context
        .content
        .encounter_for_wave(next_index)
        .expect("validated content must contain an encounter for every wave");
    let next_encounter_id = next_encounter.id.clone();

    context.stats.wave_index = next_index;
    context.stats.phase = EncounterPhase::Combat;
    context.progression_notice.0 = format!(
        "Wave {} · {}",
        next_index.saturating_add(1),
        next_encounter_id
    );
    spawn_enemy_wave(
        &mut commands,
        &context.visuals,
        &context.content,
        next_index,
    );
}
