use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};

use super::{
    arena::spawn_encounter,
    model::{
        ArenaVisuals, CombatLog, Enemy, GameSet, GameStats, Hero, HeroBrain, HeroCombat, HudText,
        Loot, ProgressionNotice, WhirlwindEffect,
    },
};
use crate::{
    content::ContentCatalog,
    gameplay::{GameBalance, Health},
    items::{Equipment, EquipmentSlot, Inventory},
    progression::{PlayerProgression, ProfilePersistence, SAVE_PATH},
    scripting::ScriptRuntime,
    stats::ActorStats,
    status_effects::StatusEffects,
};

pub(super) struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                animate_loot,
                animate_effects,
                animate_enemy_windups,
                update_hud,
            )
                .in_set(GameSet::Presentation),
        );
    }
}

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
        &'static StatusEffects,
    ),
    With<Hero>,
>;

#[derive(SystemParam)]
struct HudTargets<'w, 's> {
    profile_persistence: Res<'w, ProfilePersistence>,
    hero: HudHeroQuery<'w, 's>,
    enemies: Query<'w, 's, (&'static Health, &'static Enemy)>,
    loot: Query<'w, 's, Entity, With<Loot>>,
    text: Query<'w, 's, &'static mut Text, With<HudText>>,
    window: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        Transform::from_xyz(13.5, 16.5, 17.5).looking_at(Vec3::ZERO, Vec3::Y),
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
    spawn_ui(&mut commands);
    commands.insert_resource(visuals);
}

fn spawn_ui(commands: &mut Commands) {
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

fn animate_enemy_windups(time: Res<Time>, mut enemies: Query<(&Enemy, &mut Transform)>) {
    for (enemy, mut transform) in &mut enemies {
        let scale = if enemy.attack_windup.is_active() {
            1.08 + (time.elapsed_secs() * 16.0).sin().abs() * 0.16
        } else {
            1.0
        };
        transform.scale = Vec3::splat(scale);
    }
}

fn update_hud(
    runtime: Res<ScriptRuntime>,
    content: Res<ContentCatalog>,
    stats: Res<GameStats>,
    combat_log: Res<CombatLog>,
    progression: Res<PlayerProgression>,
    progression_notice: Res<ProgressionNotice>,
    mut targets: HudTargets,
) {
    let Ok((health, brain, combat, actor_stats, inventory, equipment, effects)) =
        targets.hero.single()
    else {
        return;
    };
    let living_enemies = targets
        .enemies
        .iter()
        .filter(|(health, _)| health.is_alive())
        .count();
    let winding_up = targets
        .enemies
        .iter()
        .filter(|(health, enemy)| health.is_alive() && enemy.attack_windup.is_active())
        .count();
    let loot_count = targets.loot.iter().count();
    let wave_number = stats.wave_index.saturating_add(1);
    let effect_labels = effects.labels().collect::<Vec<_>>();
    let effect_status = if effect_labels.is_empty() {
        "None".to_string()
    } else {
        effect_labels.join(", ")
    };
    let last_hit = combat_log.last_hit.as_ref().map_or_else(
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
    let ability_status = combat
        .abilities()
        .map(|ability| {
            format!(
                "  {:<11} {:>4.1}s",
                ability.definition.name,
                ability.remaining_cooldown()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let trace = runtime
        .trace_history()
        .rev()
        .take(3)
        .map(|entry| entry.summary())
        .collect::<Vec<_>>();
    let trace_status = if runtime.latest_trace().is_none() {
        "  No decisions recorded".to_string()
    } else {
        format!("  {}", trace.join("\n  "))
    };
    let status = format!(
        "SCRIPTED ARPG · {:?}\nWave           {} · {}\nClass          {}\n\nHero HP        {:>5.0} / {:.0}\nDamage         {:>5.1}\nDefense        {:>5.1}\nMove speed     {:>5.1}\nAttack speed   {:>5.2}x\nCurrent action {:?}\nEffects        {}\nEnemies alive {:>5}\nWinding up    {:>5}\nInterrupts    {:>5}\nLoot remaining{:>5}\nInventory     {:>5}\nScore         {:>5}\n\nProgression\n  Level   {}\n  XP      {} / {}\n  Points  {}\n  Power {} · Vitality {} · Swiftness {}\n\nProfile        {}\n  File   {}\n  [S] Save · [L] Load\n  {}\n\nEquipment\n  Weapon  {}\n  Armor   {}\n  Boots   {}\n\nAbilities\n{}\nLast hit       {}\n\nDecision trace (newest first)\n{}\n\nScript: {}",
        stats.phase,
        wave_number,
        content
            .encounter_for_wave(stats.wave_index)
            .map_or("unknown", |encounter| encounter.id.as_str()),
        combat.class_name,
        health.current(),
        health.max(),
        actor_stats.damage(),
        actor_stats.defense(),
        actor_stats.move_speed(),
        actor_stats.attack_speed(),
        brain.action,
        effect_status,
        living_enemies,
        winding_up,
        stats.interrupts,
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
        targets.profile_persistence.status_label(&progression),
        SAVE_PATH,
        progression_notice.0.as_str(),
        equipment.item_name(EquipmentSlot::Weapon).unwrap_or("—"),
        equipment.item_name(EquipmentSlot::Armor).unwrap_or("—"),
        equipment.item_name(EquipmentSlot::Boots).unwrap_or("—"),
        ability_status,
        last_hit,
        trace_status,
        runtime.status(),
    );

    if let Ok(mut text) = targets.text.single_mut() {
        **text = status;
    }
    if let Ok(mut window) = targets.window.single_mut() {
        window.title = format!(
            "Scripted ARPG MVP · Wave {} · {:?} · HP {:.0} · Score {}",
            wave_number,
            brain.action,
            health.current(),
            stats.score
        );
    }
}
