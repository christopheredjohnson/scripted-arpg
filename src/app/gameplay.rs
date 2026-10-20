use bevy::{ecs::system::SystemParam, prelude::*};

use super::{
    arena::ArenaPlugin,
    effects::StatusEffectPlugin,
    model::{
        ArenaEntity, ArenaVisuals, CombatLog, DamageResolved, DealDamage, Enemy, GameSet,
        GameStats, Hero, HeroBrain, HeroCombat, InputStage, Loot, ProgressionNotice,
        ResolutionStage, WhirlwindEffect,
    },
};
use crate::{
    content::AbilityTarget,
    gameplay::{
        ArenaBounds, DECISION_INTERVAL, Damage, DamageAbility, EncounterPhase, GameBalance, Health,
        MovementStep, nearest_target, step_away, step_toward, within_range,
    },
    items::{Equipment, Inventory},
    progression::PlayerProgression,
    scripting::{HeroAction, ScriptAbilitySnapshot, ScriptRuntime, ScriptWorldSnapshot},
    stats::{ActorStats, StatModifier, StatModifiers},
    status_effects::{ApplyStatusEffect, StatusEffectSpec, StatusEffects},
};

pub(crate) struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DealDamage>()
            .add_message::<DamageResolved>()
            .configure_sets(
                Update,
                (
                    GameSet::Input,
                    GameSet::Cooldowns,
                    GameSet::Effects,
                    GameSet::Decisions,
                    GameSet::Actions,
                    GameSet::Damage,
                    GameSet::Resolution,
                    GameSet::Presentation,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                (
                    InputStage::Controls,
                    InputStage::ArenaReset,
                    InputStage::Progression,
                )
                    .chain()
                    .in_set(GameSet::Input),
            )
            .configure_sets(
                Update,
                (
                    ResolutionStage::CombatLog,
                    ResolutionStage::Deaths,
                    ResolutionStage::WaveAdvance,
                )
                    .chain()
                    .in_set(GameSet::Resolution),
            )
            .add_plugins(StatusEffectPlugin)
            .add_plugins(ArenaPlugin)
            .add_systems(Update, tick_cooldowns.in_set(GameSet::Cooldowns))
            .add_systems(Update, run_hero_script.in_set(GameSet::Decisions))
            .add_systems(
                Update,
                (execute_hero_action, run_enemy_ai)
                    .chain()
                    .in_set(GameSet::Actions),
            )
            .add_systems(Update, apply_damage_requests.in_set(GameSet::Damage))
            .add_systems(
                Update,
                record_damage_results.in_set(ResolutionStage::CombatLog),
            );
    }
}

type EnemyTargets<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static Health), (With<Enemy>, Without<Hero>)>;

type LootTargets<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static Loot), (With<Loot>, Without<Hero>)>;

type EnemyAiQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut Enemy,
        &'static Health,
        &'static ActorStats,
        &'static StatusEffects,
    ),
    Without<Hero>,
>;

type HeroActionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Health,
        Entity,
        &'static HeroBrain,
        &'static mut HeroCombat,
        &'static mut ActorStats,
        &'static mut Inventory,
        &'static mut Equipment,
        &'static StatusEffects,
    ),
    With<Hero>,
>;

type HeroDecisionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Health,
        &'static mut HeroBrain,
        &'static HeroCombat,
        &'static ActorStats,
        &'static Inventory,
        &'static StatusEffects,
    ),
    With<Hero>,
>;

#[derive(SystemParam)]
struct HeroActionTargets<'w, 's> {
    hero: HeroActionQuery<'w, 's>,
    enemies: EnemyTargets<'w, 's>,
    loot: LootTargets<'w, 's>,
}

#[derive(SystemParam)]
struct HeroActionContext<'w> {
    time: Res<'w, Time>,
    visuals: Res<'w, ArenaVisuals>,
    balance: Res<'w, GameBalance>,
    progression: ResMut<'w, PlayerProgression>,
    stats: ResMut<'w, GameStats>,
    progression_notice: ResMut<'w, ProgressionNotice>,
}

fn tick_cooldowns(
    time: Res<Time>,
    mut hero: Query<(&mut HeroBrain, &mut HeroCombat), With<Hero>>,
    mut enemies: Query<&mut Enemy>,
) {
    let dt = time.delta_secs();
    if let Ok((mut brain, mut combat)) = hero.single_mut() {
        brain.decision_cooldown = (brain.decision_cooldown - dt).max(0.0);
        combat.tick(dt);
    }
    for mut enemy in &mut enemies {
        enemy.attack_cooldown.tick(dt);
    }
}

fn run_hero_script(
    mut runtime: ResMut<ScriptRuntime>,
    mut hero: HeroDecisionQuery,
    enemies: Query<(&Transform, &Health, &ActorStats, &Enemy)>,
    loot: Query<&Transform, With<Loot>>,
    balance: Res<GameBalance>,
    progression: Res<PlayerProgression>,
    game_stats: Res<GameStats>,
) {
    let Ok((hero_transform, hero_health, mut brain, combat, actor_stats, inventory, effects)) =
        hero.single_mut()
    else {
        return;
    };
    if !hero_health.is_alive() || brain.decision_cooldown > 0.0 {
        return;
    }

    brain.decision_cooldown = DECISION_INTERVAL;
    let hero_position = planar_position(hero_transform.translation);
    let enemy_count = i64::try_from(
        enemies
            .iter()
            .filter(|(_, health, _, _)| health.is_alive())
            .count(),
    )
    .unwrap_or(i64::MAX);
    let enemies_winding_up = i64::try_from(
        enemies
            .iter()
            .filter(|(_, health, _, enemy)| health.is_alive() && enemy.attack_windup.is_active())
            .count(),
    )
    .unwrap_or(i64::MAX);
    let nearest_enemy = nearest_target(
        hero_position,
        enemies
            .iter()
            .filter(|(_, health, _, _)| health.is_alive())
            .map(|(transform, health, stats, enemy)| {
                (
                    (
                        *health,
                        *stats,
                        enemy.attack_windup.is_active(),
                        enemy.attack_windup.remaining(),
                    ),
                    planar_position(transform.translation),
                )
            }),
    );
    let nearest_distance = nearest_enemy
        .as_ref()
        .map_or(999.0, |target| target.distance as f64);
    let (
        nearest_health_percent,
        nearest_damage,
        nearest_defense,
        nearest_move_speed,
        nearest_is_winding_up,
        nearest_windup_remaining,
    ) = nearest_enemy.map_or((0.0, 0.0, 0.0, 0.0, false, 0.0), |target| {
        let (health, stats, is_winding_up, windup_remaining) = target.id;
        (
            health.percent() as f64,
            stats.damage() as f64,
            stats.defense() as f64,
            stats.move_speed() as f64,
            is_winding_up,
            windup_remaining as f64,
        )
    });

    let loot_count = loot.iter().count();
    let loot_nearby = loot.iter().any(|transform| {
        hero_position.distance(planar_position(transform.translation))
            < balance.loot_detection_range
    });
    let snapshot = ScriptWorldSnapshot {
        class_id: combat.class_id.clone(),
        class_name: combat.class_name.clone(),
        health_percent: hero_health.percent() as f64,
        health: hero_health.current() as f64,
        max_health: hero_health.max() as f64,
        damage: actor_stats.damage() as f64,
        defense: actor_stats.defense() as f64,
        move_speed: actor_stats.move_speed() as f64,
        attack_speed: actor_stats.attack_speed() as f64,
        level: i64::from(progression.level()),
        experience: i64::from(progression.experience()),
        experience_to_next_level: i64::from(progression.experience_to_next_level()),
        unspent_points: i64::from(progression.unspent_points()),
        inventory_count: i64::try_from(inventory.len()).unwrap_or(i64::MAX),
        is_stunned: effects.is_stunned(),
        active_effects: effects.labels().map(str::to_string).collect(),
        phase: match game_stats.phase {
            EncounterPhase::Combat => "combat",
            EncounterPhase::Looting => "looting",
            EncounterPhase::Victory => "victory",
            EncounterPhase::Defeat => "defeat",
        }
        .to_string(),
        wave: i64::try_from(game_stats.wave_index.saturating_add(1)).unwrap_or(i64::MAX),
        enemy_count,
        enemies_winding_up,
        nearest_enemy_distance: nearest_distance,
        nearest_enemy_health_percent: nearest_health_percent,
        nearest_enemy_damage: nearest_damage,
        nearest_enemy_defense: nearest_defense,
        nearest_enemy_move_speed: nearest_move_speed,
        nearest_enemy_is_winding_up: nearest_is_winding_up,
        nearest_enemy_windup_remaining: nearest_windup_remaining,
        loot_count: i64::try_from(loot_count).unwrap_or(i64::MAX),
        loot_nearby,
        abilities: combat
            .abilities()
            .map(|ability| ScriptAbilitySnapshot {
                id: ability.definition.id.clone(),
                name: ability.definition.name.clone(),
                target: match ability.definition.target {
                    AbilityTarget::NearestEnemy => "nearest_enemy",
                    AbilityTarget::Area { .. } => "area",
                    AbilityTarget::SelfCast => "self",
                }
                .to_string(),
                ready: ability.is_ready(),
                cooldown: ability.remaining_cooldown() as f64,
                range: ability.range() as f64,
                targets_in_range: if matches!(ability.definition.target, AbilityTarget::SelfCast) {
                    0
                } else {
                    i64::try_from(
                        enemies
                            .iter()
                            .filter(|(transform, health, _, _)| {
                                health.is_alive()
                                    && within_range(
                                        hero_position
                                            .distance(planar_position(transform.translation)),
                                        ability.range(),
                                    )
                            })
                            .count(),
                    )
                    .unwrap_or(i64::MAX)
                },
                winding_up_targets_in_range: if matches!(
                    ability.definition.target,
                    AbilityTarget::SelfCast
                ) {
                    0
                } else {
                    i64::try_from(
                        enemies
                            .iter()
                            .filter(|(transform, health, _, enemy)| {
                                health.is_alive()
                                    && enemy.attack_windup.is_active()
                                    && within_range(
                                        hero_position
                                            .distance(planar_position(transform.translation)),
                                        ability.range(),
                                    )
                            })
                            .count(),
                    )
                    .unwrap_or(i64::MAX)
                },
            })
            .collect(),
    };

    brain.action = if effects.is_stunned() {
        let _ = runtime.decide(snapshot);
        HeroAction::Idle
    } else {
        runtime
            .decide(snapshot)
            .map_or(HeroAction::Idle, |command| command.action)
    };
}

fn execute_hero_action(
    mut commands: Commands,
    mut context: HeroActionContext,
    mut targets: HeroActionTargets,
    mut damage_requests: MessageWriter<DealDamage>,
    mut status_requests: MessageWriter<ApplyStatusEffect>,
) {
    if context.stats.phase == EncounterPhase::Defeat
        || context.stats.phase == EncounterPhase::Victory
    {
        return;
    }

    let Ok((
        mut hero_transform,
        mut health,
        hero_entity,
        brain,
        mut combat,
        mut actor_stats,
        mut inventory,
        mut equipment,
        effects,
    )) = targets.hero.single_mut()
    else {
        return;
    };
    if !health.is_alive() || effects.is_stunned() {
        return;
    }

    let dt = context.time.delta_secs();
    let hero_position = planar_position(hero_transform.translation);
    let nearest_enemy = nearest_target(
        hero_position,
        targets
            .enemies
            .iter()
            .filter(|(_, _, health)| health.is_alive())
            .map(|(entity, transform, _)| (entity, planar_position(transform.translation))),
    );

    let ability_id = match &brain.action {
        HeroAction::Cast(id) => Some(id.as_str()),
        HeroAction::Loot | HeroAction::Retreat | HeroAction::Idle => None,
    };
    if let Some(ability_id) = ability_id {
        execute_hero_ability(
            &mut commands,
            &context.visuals,
            context.balance.arena_bounds,
            dt,
            &mut hero_transform,
            hero_entity,
            &mut combat,
            *actor_stats,
            &targets.enemies,
            &mut damage_requests,
            &mut status_requests,
            ability_id,
        );
        return;
    }

    match &brain.action {
        HeroAction::Loot => {
            let nearest_loot = nearest_target(
                hero_position,
                targets.loot.iter().map(|(entity, transform, loot)| {
                    (
                        (entity, loot.item.clone()),
                        planar_position(transform.translation),
                    )
                }),
            );

            if let Some(target) = nearest_loot {
                let reached_loot = if target.is_within(context.balance.loot_pickup_range) {
                    true
                } else {
                    move_transform_toward(
                        &mut hero_transform,
                        target.position,
                        actor_stats.move_speed() * context.balance.hero.loot_move_speed_multiplier,
                        context.balance.loot_pickup_range,
                        dt,
                        context.balance.arena_bounds,
                    )
                    .reached_stopping_distance
                };

                if reached_loot {
                    let (entity, item) = target.id;
                    if inventory.is_full() {
                        commands.entity(entity).despawn();
                        context.stats.loot_collected += 1;
                        context.stats.score += item.rarity.score() / 2;
                        context.progression_notice.0 =
                            format!("Inventory full · {} salvaged", item.name);
                        return;
                    }
                    commands.entity(entity).despawn();
                    assert!(inventory.add(item.clone()));
                    assert!(context.progression.record_item(item.clone()));
                    if equipment.equip_if_upgrade(&item) {
                        context
                            .progression
                            .set_equipped_item(item.slot, item.instance_id.clone());
                        actor_stats
                            .set_modifiers(equipment.modifiers() + context.progression.modifiers());
                        health.set_max_preserving_percentage(actor_stats.max_health());
                    }
                    context.stats.loot_collected += 1;
                    context.stats.score += item.rarity.score();
                }
            }
        }
        HeroAction::Retreat => {
            if let Some(target) = nearest_enemy {
                apply_movement(
                    &mut hero_transform,
                    step_away(
                        hero_position,
                        target.position,
                        actor_stats.move_speed()
                            * context.balance.hero.retreat_move_speed_multiplier,
                        dt,
                        context.balance.arena_bounds,
                    ),
                );
            }
            status_requests.write(ApplyStatusEffect {
                source: hero_entity,
                target: hero_entity,
                effect: StatusEffectSpec::regeneration(
                    "retreat_regeneration",
                    0.4,
                    context.balance.hero.retreat_healing_per_second,
                ),
            });
            status_requests.write(ApplyStatusEffect {
                source: hero_entity,
                target: hero_entity,
                effect: StatusEffectSpec::stat_modifier(
                    "retreat_guard",
                    0.4,
                    StatModifiers {
                        defense: StatModifier {
                            flat: 20.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            });
        }
        HeroAction::Idle => {}
        HeroAction::Cast(_) => unreachable!(),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_hero_ability(
    commands: &mut Commands,
    visuals: &ArenaVisuals,
    arena_bounds: ArenaBounds,
    dt: f32,
    hero_transform: &mut Transform,
    hero_entity: Entity,
    combat: &mut HeroCombat,
    actor_stats: ActorStats,
    enemies: &EnemyTargets,
    damage_requests: &mut MessageWriter<DealDamage>,
    status_requests: &mut MessageWriter<ApplyStatusEffect>,
    ability_id: &str,
) {
    let Some(ability) = combat.ability(ability_id) else {
        return;
    };
    let definition = ability.definition.clone();
    let damage = ability.damage(actor_stats);
    let ready = ability.is_ready();
    let hero_position = planar_position(hero_transform.translation);

    let nearest_enemy = || {
        nearest_target(
            hero_position,
            enemies
                .iter()
                .filter(|(_, _, health)| health.is_alive())
                .map(|(entity, transform, _)| (entity, planar_position(transform.translation))),
        )
    };

    let targets = match definition.target {
        AbilityTarget::NearestEnemy => {
            let Some(target) = nearest_enemy() else {
                return;
            };
            if !target.is_within(definition.range) {
                move_transform_toward(
                    hero_transform,
                    target.position,
                    actor_stats.move_speed() * definition.move_speed_multiplier,
                    definition.range,
                    dt,
                    arena_bounds,
                );
                return;
            }
            vec![target.id]
        }
        AbilityTarget::Area {
            max_targets,
            approach,
        } => {
            let mut targets = enemies
                .iter()
                .filter(|(_, transform, health)| {
                    health.is_alive()
                        && within_range(
                            hero_position.distance(planar_position(transform.translation)),
                            definition.range,
                        )
                })
                .map(|(entity, transform, _)| {
                    (
                        hero_position.distance(planar_position(transform.translation)),
                        entity,
                    )
                })
                .collect::<Vec<_>>();
            targets.sort_by(|left, right| left.0.total_cmp(&right.0));
            if let Some(limit) = max_targets {
                targets.truncate(limit);
            }
            if targets.is_empty()
                && approach
                && let Some(target) = nearest_enemy()
            {
                move_transform_toward(
                    hero_transform,
                    target.position,
                    actor_stats.move_speed() * definition.move_speed_multiplier,
                    definition.range,
                    dt,
                    arena_bounds,
                );
                return;
            }
            targets.into_iter().map(|(_, entity)| entity).collect()
        }
        AbilityTarget::SelfCast => Vec::new(),
    };

    let needs_target = !matches!(definition.target, AbilityTarget::SelfCast);
    if !ready || (needs_target && targets.is_empty()) {
        return;
    }
    let Some(ability) = combat.ability_mut(ability_id) else {
        return;
    };
    if !ability.activate(actor_stats.attack_speed()) {
        return;
    }

    for target in targets {
        if damage > 0.0 {
            damage_requests.write(DealDamage {
                source: hero_entity,
                target,
                damage: Damage::new(DamageAbility::HeroAbility(definition.name.clone()), damage),
            });
        }
        for effect in &definition.target_effects {
            status_requests.write(ApplyStatusEffect {
                source: hero_entity,
                target,
                effect: effect.clone(),
            });
        }
    }
    for effect in &definition.self_effects {
        status_requests.write(ApplyStatusEffect {
            source: hero_entity,
            target: hero_entity,
            effect: effect.clone(),
        });
    }

    if matches!(definition.target, AbilityTarget::Area { .. }) {
        commands.spawn((
            ArenaEntity,
            WhirlwindEffect {
                remaining: 0.45,
                total: 0.45,
            },
            Mesh3d(visuals.ring_mesh.clone()),
            MeshMaterial3d(visuals.ring_material.clone()),
            Transform::from_xyz(hero_position.x, 0.16, hero_position.y)
                .with_scale(Vec3::splat(0.25)),
        ));
    }
}

fn run_enemy_ai(
    time: Res<Time>,
    mut stats: ResMut<GameStats>,
    balance: Res<GameBalance>,
    hero: Query<(Entity, &Transform, &Health), With<Hero>>,
    mut enemies: EnemyAiQuery,
    mut damage_requests: MessageWriter<DealDamage>,
    mut status_requests: MessageWriter<ApplyStatusEffect>,
) {
    if stats.phase != EncounterPhase::Combat {
        return;
    }

    let Ok((hero_entity, hero_transform, hero_health)) = hero.single() else {
        return;
    };
    if !hero_health.is_alive() {
        return;
    }

    let hero_position = planar_position(hero_transform.translation);
    let dt = time.delta_secs();
    for (enemy_entity, mut transform, mut enemy, health, actor_stats, effects) in &mut enemies {
        if !health.is_alive() {
            continue;
        }
        if effects.is_stunned() {
            if enemy.attack_windup.cancel() {
                stats.interrupts = stats.interrupts.saturating_add(1);
            }
            continue;
        }

        let distance = planar_position(transform.translation).distance(hero_position);
        if enemy.attack_windup.is_active() {
            if !within_range(distance, enemy.attack_range) {
                enemy.attack_windup.cancel();
                continue;
            }

            enemy.attack_windup.tick(dt);
            if enemy.attack_windup.is_complete() {
                enemy.attack_windup.cancel();
                damage_requests.write(DealDamage {
                    source: enemy_entity,
                    target: hero_entity,
                    damage: Damage::new(DamageAbility::EnemyMelee, actor_stats.damage()),
                });
                if let Some(effect) = &enemy.on_hit_effect {
                    status_requests.write(ApplyStatusEffect {
                        source: enemy_entity,
                        target: hero_entity,
                        effect: effect.clone(),
                    });
                }
                let cooldown_duration = enemy.attack_cooldown_duration / actor_stats.attack_speed();
                enemy.attack_cooldown.start(cooldown_duration);
            }
            continue;
        }

        if !within_range(distance, enemy.attack_range) {
            move_transform_toward(
                &mut transform,
                hero_position,
                actor_stats.move_speed(),
                enemy.attack_range,
                dt,
                balance.arena_bounds,
            );
        } else if enemy.attack_cooldown.is_ready() {
            let windup_duration = enemy.attack_windup_duration / actor_stats.attack_speed();
            enemy.attack_windup.start(windup_duration);
        }
    }
}

fn apply_damage_requests(
    mut requests: MessageReader<DealDamage>,
    mut targets: Query<(&mut Health, &ActorStats)>,
    mut results: MessageWriter<DamageResolved>,
) {
    for request in requests.read() {
        let Ok((mut target_health, target_stats)) = targets.get_mut(request.target) else {
            continue;
        };
        let result = target_health.apply_damage(request.damage.clone(), target_stats.defense());
        results.write(DamageResolved {
            source: request.source,
            target: request.target,
            result,
        });
    }
}

fn record_damage_results(
    mut results: MessageReader<DamageResolved>,
    mut combat_log: ResMut<CombatLog>,
) {
    for result in results.read() {
        combat_log.last_hit = Some(result.clone());
    }
}

fn move_transform_toward(
    transform: &mut Transform,
    target: Vec2,
    speed: f32,
    stopping_distance: f32,
    delta_seconds: f32,
    bounds: ArenaBounds,
) -> MovementStep {
    let movement = step_toward(
        planar_position(transform.translation),
        target,
        speed,
        stopping_distance,
        delta_seconds,
        bounds,
    );
    apply_movement(transform, movement);
    movement
}

fn apply_movement(transform: &mut Transform, movement: MovementStep) {
    transform.translation.x = movement.position.x;
    transform.translation.z = movement.position.y;
    if movement.direction.length_squared() > 0.0 {
        transform.look_to(
            Vec3::new(movement.direction.x, 0.0, movement.direction.y),
            Vec3::Y,
        );
    }
}

fn planar_position(position: Vec3) -> Vec2 {
    Vec2::new(position.x, position.z)
}
