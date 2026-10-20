use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};

use super::{
    arena::{equipment_from_profile, validate_profile_items},
    model::{GameStats, Hero, HeroCombat, InputStage, ProgressionNotice},
};
use crate::{
    content::ContentCatalog,
    gameplay::Health,
    items::{Equipment, Inventory},
    progression::{
        PlayerProgression, ProfileLoadError, ProfilePersistence, SAVE_PATH, UpgradeKind,
    },
    scripting::ScriptRuntime,
    stats::ActorStats,
};

pub(super) struct ControlsPlugin;

const LOAD_CONFIRM_SECONDS: f32 = 3.0;

#[derive(Default)]
struct LoadConfirmation {
    remaining: f32,
}

type ProgressionHeroQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ActorStats,
        &'static mut Health,
        &'static mut Inventory,
        &'static mut Equipment,
        &'static mut HeroCombat,
    ),
    With<Hero>,
>;

#[derive(SystemParam)]
struct ProgressionControlsContext<'w, 's> {
    time: Res<'w, Time>,
    progression: ResMut<'w, PlayerProgression>,
    persistence: ResMut<'w, ProfilePersistence>,
    notice: ResMut<'w, ProgressionNotice>,
    content: Res<'w, ContentCatalog>,
    hero: ProgressionHeroQuery<'w, 's>,
}

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (keyboard_controls, watch_script_changes).in_set(InputStage::Controls),
        )
        .add_systems(Update, progression_controls.in_set(InputStage::Progression));
    }
}

fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ScriptRuntime>,
    mut stats: ResMut<GameStats>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        runtime.reload();
    }

    if keys.just_pressed(KeyCode::KeyT) {
        runtime.clear_trace();
    }

    if keys.just_pressed(KeyCode::Space) {
        stats.reset_requested = true;
    }

    if keys.just_pressed(KeyCode::Escape) {
        app_exit.write(AppExit::Success);
    }
}

fn watch_script_changes(
    time: Res<Time>,
    mut elapsed: Local<f32>,
    mut runtime: ResMut<ScriptRuntime>,
) {
    *elapsed += time.delta_secs();
    if *elapsed < 0.25 {
        return;
    }
    *elapsed = 0.0;

    runtime.reload_if_changed();
}

fn progression_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut load_confirmation: Local<LoadConfirmation>,
    mut context: ProgressionControlsContext,
) {
    load_confirmation.remaining =
        (load_confirmation.remaining - context.time.delta_secs()).max(0.0);

    if keys.just_pressed(KeyCode::KeyS) {
        load_confirmation.remaining = 0.0;
        match context.progression.save() {
            Ok(()) => {
                context.persistence.mark_saved(&context.progression);
                context.notice.0 = format!(
                    "SAVED · Level {} · {} item{} · {SAVE_PATH}",
                    context.progression.level(),
                    context.progression.items().len(),
                    if context.progression.items().len() == 1 {
                        ""
                    } else {
                        "s"
                    },
                );
            }
            Err(error) => {
                context.persistence.mark_error();
                context.notice.0 = format!("SAVE ERROR · Progress remains in memory · {error}");
            }
        }
    }

    let mut build_changed = false;
    let mut profile_loaded = false;
    if keys.just_pressed(KeyCode::KeyL) {
        if context
            .persistence
            .load_requires_confirmation(&context.progression)
            && load_confirmation.remaining <= 0.0
        {
            load_confirmation.remaining = LOAD_CONFIRM_SECONDS;
            context.notice.0 =
                "UNSAVED CHANGES · Press L again within 3 seconds to discard and load".to_string();
        } else {
            load_confirmation.remaining = 0.0;
            match PlayerProgression::load(&context.content) {
                Ok(loaded) => match validate_profile_items(&loaded, &context.content) {
                    Ok(()) => {
                        *context.progression = loaded;
                        context.persistence.mark_saved(&context.progression);
                        context.notice.0 = format!(
                            "LOADED · Level {} · {} item{} · current run replaced",
                            context.progression.level(),
                            context.progression.items().len(),
                            if context.progression.items().len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                        );
                        build_changed = true;
                        profile_loaded = true;
                    }
                    Err(error) => {
                        context.persistence.mark_error();
                        context.notice.0 = format!("LOAD ERROR · Current run kept · {error}");
                    }
                },
                Err(ProfileLoadError::NotFound(_)) => {
                    context.persistence.mark_missing();
                    context.notice.0 =
                        "LOAD · No saved profile found; current run kept".to_string();
                }
                Err(error) => {
                    context.persistence.mark_error();
                    context.notice.0 = format!("LOAD ERROR · Current run kept · {error}");
                }
            }
        }
    }

    for (key, upgrade) in [
        (KeyCode::Digit1, UpgradeKind::Power),
        (KeyCode::Digit2, UpgradeKind::Vitality),
        (KeyCode::Digit3, UpgradeKind::Swiftness),
    ] {
        if keys.just_pressed(key) {
            if context.progression.spend_point(upgrade) {
                context.notice.0 = format!("Upgraded {}", upgrade.label());
                build_changed = true;
            } else {
                context.notice.0 = "No upgrade points available".to_string();
            }
        }
    }

    if build_changed
        && let Ok((mut actor_stats, mut health, mut inventory, mut equipment, mut combat)) =
            context.hero.single_mut()
    {
        *inventory = Inventory::from_items(context.progression.items().to_vec());
        *equipment = equipment_from_profile(&context.progression);
        actor_stats.set_modifiers(equipment.modifiers() + context.progression.modifiers());
        health.set_max_preserving_percentage(actor_stats.max_health());
        if profile_loaded {
            *combat = HeroCombat::from_profile(&context.content, &context.progression)
                .expect("loaded profile class and abilities must be valid");
        }
    }
}
