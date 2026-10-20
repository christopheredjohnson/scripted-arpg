mod content;
mod gameplay;
mod items;
mod progression;
mod scripting;
mod stats;
#[cfg(test)]
mod tests;

use bevy::{
    app::AppExit,
    ecs::system::SystemParam,
    prelude::*,
    window::{PrimaryWindow, WindowResolution},
};
use content::ContentCatalog;

use gameplay::{
    Ability, Cooldown, DECISION_INTERVAL, Damage, DamageAbility, DamageResult, EncounterPhase,
    EncounterSnapshot, GameBalance, Health, MovementStep, nearest_target, resolve_encounter_phase,
    step_away, step_toward, within_range,
};
use items::{Equipment, EquipmentSlot, Inventory, ItemDefinition, LootRarity};
use progression::{PlayerProgression, UpgradeKind};
use scripting::{HERO_SCRIPT, HeroAction, ScriptRuntime, ScriptWorldSnapshot};
use stats::ActorStats;

fn main() {
    let content = ContentCatalog::load()
        .unwrap_or_else(|error| panic!("failed to load game content: {error}"));
    let (progression, progression_notice) = match PlayerProgression::load() {
        Ok(progression) => (progression, "Loaded saved progression".to_string()),
        Err(_) => (
            PlayerProgression::default(),
            "New profile · [S] Save · [L] Load".to_string(),
        ),
    };
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.018, 0.022, 0.035)))
        .insert_resource(GameBalance::default())
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(progression)
        .insert_resource(ProgressionNotice(progression_notice))
        .insert_resource(content)
        .insert_resource(ScriptRuntime::new())
        .add_message::<DealDamage>()
        .add_message::<DamageResolved>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Scripted ARPG MVP".into(),
                resolution: WindowResolution::new(1440, 900),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                keyboard_controls,
                reset_arena,
                progression_controls,
                tick_cooldowns,
                run_hero_script,
                execute_hero_action,
                run_enemy_ai,
                apply_damage_requests,
                record_damage_results,
                resolve_deaths,
                animate_loot,
                animate_effects,
                update_hud,
            )
                .chain(),
        )
        .run();
}

#[derive(Component)]
struct Hero;

#[derive(Component)]
struct Enemy {
    attack_cooldown: Cooldown,
    attack_range: f32,
    attack_cooldown_duration: f32,
    drop_item: ItemDefinition,
    xp_reward: u32,
}

#[derive(Component)]
struct HeroBrain {
    action: HeroAction,
    decision_cooldown: f32,
}

#[derive(Component)]
struct HeroCombat {
    basic: Ability,
    whirlwind: Ability,
}

#[derive(Component)]
struct Loot {
    item: ItemDefinition,
    base_y: f32,
    phase: f32,
}

#[derive(Component)]
struct ArenaEntity;

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct ScriptText;

#[derive(Component)]
struct WhirlwindEffect {
    remaining: f32,
    total: f32,
}

#[derive(Message, Clone, Copy, Debug)]
struct DealDamage {
    source: Entity,
    target: Entity,
    damage: Damage,
}

#[derive(Message, Clone, Copy, Debug)]
struct DamageResolved {
    source: Entity,
    target: Entity,
    result: DamageResult,
}

#[derive(Resource, Default)]
struct CombatLog {
    last_hit: Option<DamageResolved>,
}

#[derive(Resource)]
struct ProgressionNotice(String);

type EnemyTargets<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static Health), (With<Enemy>, Without<Hero>)>;

type LootTargets<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static Loot), (With<Loot>, Without<Hero>)>;

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
    ),
    With<Hero>,
>;

type HudHeroQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Health,
        &'static HeroBrain,
        &'static HeroCombat,
        &'static ActorStats,
        &'static Inventory,
        &'static Equipment,
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
struct HudTargets<'w, 's> {
    hero: HudHeroQuery<'w, 's>,
    enemies: Query<'w, 's, &'static Health, With<Enemy>>,
    loot: Query<'w, 's, Entity, With<Loot>>,
    text: Query<'w, 's, &'static mut Text, With<HudText>>,
    window: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
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
struct HeroActionContext<'w> {
    time: Res<'w, Time>,
    visuals: Res<'w, ArenaVisuals>,
    balance: Res<'w, GameBalance>,
    progression: Res<'w, PlayerProgression>,
    stats: ResMut<'w, GameStats>,
}

#[derive(SystemParam)]
struct DeathContext<'w> {
    visuals: Res<'w, ArenaVisuals>,
    stats: ResMut<'w, GameStats>,
    progression: ResMut<'w, PlayerProgression>,
    progression_notice: ResMut<'w, ProgressionNotice>,
}

#[derive(Resource)]
struct GameStats {
    kills: u32,
    loot_collected: u32,
    score: u32,
    phase: EncounterPhase,
    reset_requested: bool,
}

impl Default for GameStats {
    fn default() -> Self {
        Self {
            kills: 0,
            loot_collected: 0,
            score: 0,
            phase: EncounterPhase::Combat,
            reset_requested: false,
        }
    }
}

#[derive(Resource, Clone)]
struct ArenaVisuals {
    hero_mesh: Handle<Mesh>,
    enemy_mesh: Handle<Mesh>,
    loot_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    hero_material: Handle<StandardMaterial>,
    enemy_material: Handle<StandardMaterial>,
    common_material: Handle<StandardMaterial>,
    rare_material: Handle<StandardMaterial>,
    epic_material: Handle<StandardMaterial>,
    ring_material: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    runtime: Res<ScriptRuntime>,
    balance: Res<GameBalance>,
    content: Res<ContentCatalog>,
    progression: Res<PlayerProgression>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.32, 0.38, 0.55),
        brightness: 180.0,
        ..default()
    });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(13.5, 16.5, 17.5).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.7, 0.0)),
    ));

    let floor_material = materials.add(Color::srgb(0.06, 0.07, 0.10));
    let wall_material = materials.add(Color::srgb(0.11, 0.12, 0.16));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(22.0, 22.0))),
        MeshMaterial3d(floor_material),
    ));

    let wall_mesh = meshes.add(Cuboid::new(22.0, 0.7, 0.55));
    for (translation, rotation) in [
        (Vec3::new(0.0, 0.35, -11.0), 0.0),
        (Vec3::new(0.0, 0.35, 11.0), 0.0),
        (Vec3::new(-11.0, 0.35, 0.0), std::f32::consts::FRAC_PI_2),
        (Vec3::new(11.0, 0.35, 0.0), std::f32::consts::FRAC_PI_2),
    ] {
        commands.spawn((
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(translation).with_rotation(Quat::from_rotation_y(rotation)),
        ));
    }

    let visuals = ArenaVisuals {
        hero_mesh: meshes.add(Capsule3d::new(0.48, 1.1)),
        enemy_mesh: meshes.add(Capsule3d::new(0.4, 0.75)),
        loot_mesh: meshes.add(Cuboid::new(0.42, 0.42, 0.42)),
        ring_mesh: meshes.add(Torus::new(2.7, 3.0)),
        hero_material: materials.add(Color::srgb(0.08, 0.62, 1.0)),
        enemy_material: materials.add(Color::srgb(0.82, 0.08, 0.11)),
        common_material: materials.add(Color::srgb(0.72, 0.74, 0.78)),
        rare_material: materials.add(Color::srgb(0.12, 0.42, 1.0)),
        epic_material: materials.add(Color::srgb(0.72, 0.16, 1.0)),
        ring_material: materials.add(Color::srgb(0.05, 0.82, 1.0)),
    };

    spawn_encounter(&mut commands, &visuals, &balance, &content, &progression);
    spawn_ui(&mut commands, runtime.source());
    commands.insert_resource(visuals);
}

fn spawn_ui(commands: &mut Commands, script_source: &str) {
    commands.spawn((
        HudText,
        Text::new("Starting encounter..."),
        TextFont {
            font_size: FontSize::Px(21.0),
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            left: px(18),
            width: px(430),
            padding: UiRect::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.02, 0.035, 0.92)),
    ));

    commands.spawn((
        ScriptText,
        Text::new(format!(
            "HERO PROGRAM · {HERO_SCRIPT}\n\n{}\n\n[R] Reload   [Space] Restart   [S] Save   [L] Load\n[1] Power   [2] Vitality   [3] Swiftness   [Esc] Quit",
            script_source.trim()
        )),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.72, 0.88, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            right: px(18),
            width: px(545),
            padding: UiRect::all(px(16)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.015, 0.028, 0.94)),
    ));
}

fn spawn_encounter(
    commands: &mut Commands,
    visuals: &ArenaVisuals,
    balance: &GameBalance,
    content: &ContentCatalog,
    progression: &PlayerProgression,
) {
    let hero_stats = balance.hero.stats.with_modifiers(progression.modifiers());
    commands.spawn((
        ArenaEntity,
        Hero,
        Health::new(hero_stats.max_health()),
        hero_stats,
        HeroBrain {
            action: HeroAction::Idle,
            decision_cooldown: 0.0,
        },
        HeroCombat {
            basic: Ability::new(balance.hero.attack),
            whirlwind: Ability::new(balance.hero.whirlwind),
        },
        Inventory::default(),
        Equipment::default(),
        Mesh3d(visuals.hero_mesh.clone()),
        MeshMaterial3d(visuals.hero_material.clone()),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));

    let encounter = content.starting_encounter();
    for group_id in &encounter.spawn_groups {
        let group = content
            .spawn_group(group_id)
            .expect("validated encounter must reference a known spawn group");
        for (index, spawn) in group.enemies.iter().enumerate() {
            let archetype = content
                .enemy_archetype(&spawn.archetype)
                .expect("validated spawn must reference a known enemy archetype");
            let actor_stats = archetype.actor_stats();
            let drop_item = content
                .item(&archetype.drop_item)
                .expect("validated archetype must reference a known item")
                .clone();
            let mut attack_cooldown = Cooldown::default();
            attack_cooldown.start(index as f32 * group.attack_stagger);

            commands.spawn((
                ArenaEntity,
                Enemy {
                    attack_cooldown,
                    attack_range: archetype.attack_range,
                    attack_cooldown_duration: archetype.attack_cooldown,
                    drop_item,
                    xp_reward: archetype.xp_reward,
                },
                Health::new(actor_stats.max_health()),
                actor_stats,
                Mesh3d(visuals.enemy_mesh.clone()),
                MeshMaterial3d(visuals.enemy_material.clone()),
                Transform::from_xyz(spawn.x, 0.8, spawn.z),
            ));
        }
    }
}

fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ScriptRuntime>,
    mut stats: ResMut<GameStats>,
    mut script_text: Query<&mut Text, With<ScriptText>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        runtime.reload();
        if let Ok(mut text) = script_text.single_mut() {
            **text = format!(
                "HERO PROGRAM · {HERO_SCRIPT}\n\n{}\n\n[R] Reload   [Space] Restart   [S] Save   [L] Load\n[1] Power   [2] Vitality   [3] Swiftness   [Esc] Quit",
                runtime.source().trim()
            );
        }
    }

    if keys.just_pressed(KeyCode::Space) {
        stats.reset_requested = true;
    }

    if keys.just_pressed(KeyCode::Escape) {
        app_exit.write(AppExit::Success);
    }
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

fn progression_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut progression: ResMut<PlayerProgression>,
    mut notice: ResMut<ProgressionNotice>,
    mut hero: Query<(&mut ActorStats, &mut Health, &Equipment), With<Hero>>,
) {
    if keys.just_pressed(KeyCode::KeyS) {
        notice.0 = match progression.save() {
            Ok(()) => "Progression saved".to_string(),
            Err(error) => format!("SAVE ERROR · {error}"),
        };
    }

    let mut build_changed = false;
    if keys.just_pressed(KeyCode::KeyL) {
        match PlayerProgression::load() {
            Ok(loaded) => {
                *progression = loaded;
                notice.0 = "Progression loaded".to_string();
                build_changed = true;
            }
            Err(error) => notice.0 = format!("LOAD ERROR · {error}"),
        }
    }

    for (key, upgrade) in [
        (KeyCode::Digit1, UpgradeKind::Power),
        (KeyCode::Digit2, UpgradeKind::Vitality),
        (KeyCode::Digit3, UpgradeKind::Swiftness),
    ] {
        if keys.just_pressed(key) {
            if progression.spend_point(upgrade) {
                notice.0 = format!("Upgraded {}", upgrade.label());
                build_changed = true;
            } else {
                notice.0 = "No upgrade points available".to_string();
            }
        }
    }

    if build_changed && let Ok((mut actor_stats, mut health, equipment)) = hero.single_mut() {
        actor_stats.set_modifiers(equipment.modifiers() + progression.modifiers());
        health.set_max_preserving_percentage(actor_stats.max_health());
    }
}

fn tick_cooldowns(
    time: Res<Time>,
    mut hero: Query<(&mut HeroBrain, &mut HeroCombat), With<Hero>>,
    mut enemies: Query<&mut Enemy>,
) {
    let dt = time.delta_secs();
    if let Ok((mut brain, mut combat)) = hero.single_mut() {
        brain.decision_cooldown = (brain.decision_cooldown - dt).max(0.0);
        combat.basic.tick(dt);
        combat.whirlwind.tick(dt);
    }
    for mut enemy in &mut enemies {
        enemy.attack_cooldown.tick(dt);
    }
}

fn run_hero_script(
    mut runtime: ResMut<ScriptRuntime>,
    mut hero: Query<(&Transform, &Health, &mut HeroBrain, &HeroCombat), With<Hero>>,
    enemies: Query<(&Transform, &Health), With<Enemy>>,
    loot: Query<&Transform, With<Loot>>,
    balance: Res<GameBalance>,
) {
    let Ok((hero_transform, hero_health, mut brain, combat)) = hero.single_mut() else {
        return;
    };
    if !hero_health.is_alive() || brain.decision_cooldown > 0.0 {
        return;
    }

    brain.decision_cooldown = DECISION_INTERVAL;
    let hero_position = planar_position(hero_transform.translation);
    let enemy_count = enemies
        .iter()
        .filter(|(_, health)| health.is_alive())
        .count() as i64;
    let nearest_distance = nearest_target(
        hero_position,
        enemies
            .iter()
            .filter(|(_, health)| health.is_alive())
            .map(|(transform, _)| ((), planar_position(transform.translation))),
    )
    .map_or(999.0, |target| target.distance as f64);

    let loot_nearby = loot.iter().any(|transform| {
        hero_position.distance(planar_position(transform.translation))
            < balance.loot_detection_range
    });
    let health_percent = hero_health.percent() as f64;

    let snapshot = ScriptWorldSnapshot {
        health_percent,
        enemy_count,
        nearest_enemy_distance: nearest_distance,
        whirlwind_ready: combat.whirlwind.is_ready(),
        loot_nearby,
    };

    match runtime.decide(snapshot) {
        Ok(command) => brain.action = command.action,
        Err(_) => brain.action = HeroAction::Idle,
    }
}

fn execute_hero_action(
    mut commands: Commands,
    mut context: HeroActionContext,
    mut targets: HeroActionTargets,
    mut damage_requests: MessageWriter<DealDamage>,
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
    )) = targets.hero.single_mut()
    else {
        return;
    };
    if !health.is_alive() {
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

    match brain.action {
        HeroAction::Attack => {
            if let Some(target) = nearest_enemy {
                if !target.is_within(combat.basic.range()) {
                    move_transform_toward(
                        &mut hero_transform,
                        target.position,
                        actor_stats.move_speed(),
                        combat.basic.range(),
                        dt,
                        context.balance.arena_bounds,
                    );
                } else if combat.basic.activate(actor_stats.attack_speed()) {
                    damage_requests.write(DealDamage {
                        source: hero_entity,
                        target: target.id,
                        damage: Damage::new(
                            DamageAbility::BasicAttack,
                            combat.basic.damage(*actor_stats),
                        ),
                    });
                }
            }
        }
        HeroAction::Whirlwind => {
            if combat.whirlwind.is_ready() {
                let mut hit_anything = false;
                for (enemy_entity, enemy_transform, enemy_health) in &targets.enemies {
                    if enemy_health.is_alive()
                        && within_range(
                            hero_position.distance(planar_position(enemy_transform.translation)),
                            combat.whirlwind.range(),
                        )
                    {
                        damage_requests.write(DealDamage {
                            source: hero_entity,
                            target: enemy_entity,
                            damage: Damage::new(
                                DamageAbility::Whirlwind,
                                combat.whirlwind.damage(*actor_stats),
                            ),
                        });
                        hit_anything = true;
                    }
                }
                if hit_anything {
                    combat.whirlwind.activate(actor_stats.attack_speed());
                    commands.spawn((
                        ArenaEntity,
                        WhirlwindEffect {
                            remaining: 0.45,
                            total: 0.45,
                        },
                        Mesh3d(context.visuals.ring_mesh.clone()),
                        MeshMaterial3d(context.visuals.ring_material.clone()),
                        Transform::from_xyz(hero_position.x, 0.16, hero_position.y)
                            .with_scale(Vec3::splat(0.25)),
                    ));
                }
            }
        }
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
                    commands.entity(entity).despawn();
                    inventory.add(item.id.clone());
                    if equipment.equip_if_upgrade(&item) {
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
            // Retreat represents a defensive recovery action in the MVP. It gives the
            // example program a way to stabilize without adding an inventory system yet.
            health.heal(context.balance.hero.retreat_healing_per_second * dt);
        }
        HeroAction::Idle => {}
    }
}

fn run_enemy_ai(
    time: Res<Time>,
    stats: Res<GameStats>,
    balance: Res<GameBalance>,
    hero: Query<(Entity, &Transform, &Health), With<Hero>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, &Health, &ActorStats), Without<Hero>>,
    mut damage_requests: MessageWriter<DealDamage>,
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
    for (enemy_entity, mut transform, mut enemy, health, actor_stats) in &mut enemies {
        if !health.is_alive() {
            continue;
        }
        let distance = planar_position(transform.translation).distance(hero_position);
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
            damage_requests.write(DealDamage {
                source: enemy_entity,
                target: hero_entity,
                damage: Damage::new(DamageAbility::EnemyMelee, actor_stats.damage()),
            });
            let cooldown_duration = enemy.attack_cooldown_duration / actor_stats.attack_speed();
            enemy.attack_cooldown.start(cooldown_duration);
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
        let result = target_health.apply_damage(request.damage, target_stats.defense());
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
        combat_log.last_hit = Some(*result);
    }
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

        let item = enemy.drop_item.clone();
        let rarity = item.rarity;
        let material = match rarity {
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

fn animate_loot(time: Res<Time>, mut loot: Query<(&mut Transform, &Loot)>) {
    for (mut transform, loot) in &mut loot {
        let wave = (time.elapsed_secs() * 3.0 + loot.phase).sin() * 0.12;
        transform.translation.y = loot.base_y + wave;
        transform.rotate_y(time.delta_secs() * 1.8);
    }
}

fn animate_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut Transform, &mut WhirlwindEffect)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut effect) in &mut effects {
        effect.remaining -= dt;
        if effect.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = 1.0 - effect.remaining / effect.total;
        transform.scale = Vec3::splat(0.25 + progress * 0.9);
        transform.rotate_y(dt * 5.0);
    }
}

fn update_hud(
    runtime: Res<ScriptRuntime>,
    stats: Res<GameStats>,
    combat_log: Res<CombatLog>,
    progression: Res<PlayerProgression>,
    progression_notice: Res<ProgressionNotice>,
    mut targets: HudTargets,
) {
    let Ok((health, brain, combat, actor_stats, inventory, equipment)) = targets.hero.single()
    else {
        return;
    };
    let living_enemies = targets
        .enemies
        .iter()
        .filter(|health| health.is_alive())
        .count();
    let loot_count = targets.loot.iter().count();
    let last_hit = combat_log.last_hit.map_or_else(
        || "None".to_string(),
        |hit| {
            format!(
                "{} · {:.0} damage{} · {:?} → {:?}",
                hit.result.damage.ability.label(),
                hit.result.applied,
                if hit.result.killed { " · KILL" } else { "" },
                hit.source,
                hit.target,
            )
        },
    );
    let status = format!(
        "SCRIPTED ARPG · {:?}\n\nHero HP        {:>5.0} / {:.0}\nDamage         {:>5.1}\nDefense        {:>5.1}\nMove speed     {:>5.1}\nAttack speed   {:>5.2}x\nCurrent action {:?}\nEnemies alive {:>5}\nLoot remaining{:>5}\nInventory     {:>5}\nScore         {:>5}\n\nProgression\n  Level   {}\n  XP      {} / {}\n  Points  {}\n  Power {} · Vitality {} · Swiftness {}\n\nEquipment\n  Weapon  {}\n  Armor   {}\n  Boots   {}\n\nWhirlwind CD  {:>5.1}s\nLast hit       {}\n\n{}\nScript: {}",
        stats.phase,
        health.current(),
        health.max(),
        actor_stats.damage(),
        actor_stats.defense(),
        actor_stats.move_speed(),
        actor_stats.attack_speed(),
        brain.action,
        living_enemies,
        loot_count,
        inventory.len(),
        stats.score,
        progression.level(),
        progression.experience(),
        progression.experience_to_next_level(),
        progression.unspent_points(),
        progression.upgrades().power(),
        progression.upgrades().vitality(),
        progression.upgrades().swiftness(),
        equipment.item_name(EquipmentSlot::Weapon).unwrap_or("—"),
        equipment.item_name(EquipmentSlot::Armor).unwrap_or("—"),
        equipment.item_name(EquipmentSlot::Boots).unwrap_or("—"),
        combat.whirlwind.remaining_cooldown(),
        last_hit,
        progression_notice.0.as_str(),
        runtime.status(),
    );

    if let Ok(mut text) = targets.text.single_mut() {
        **text = status;
    }
    if let Ok(mut window) = targets.window.single_mut() {
        window.title = format!(
            "Scripted ARPG MVP · {:?} · HP {:.0} · Score {}",
            brain.action,
            health.current(),
            stats.score
        );
    }
}

fn move_transform_toward(
    transform: &mut Transform,
    target: Vec2,
    speed: f32,
    stopping_distance: f32,
    delta_seconds: f32,
    bounds: gameplay::ArenaBounds,
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
