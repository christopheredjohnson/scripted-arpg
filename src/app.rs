mod arena;
mod controls;
mod effects;
mod gameplay;
mod model;
mod presentation;

use bevy::{
    prelude::*,
    window::{Window, WindowPlugin, WindowResolution},
};

use crate::{
    content::ContentCatalog,
    gameplay::GameBalance,
    progression::{PlayerProgression, ProfileLoadError, ProfilePersistence},
    scripting::ScriptRuntime,
};
use arena::validate_profile_items;
use controls::ControlsPlugin;
pub(crate) use gameplay::GameplayPlugin;
#[cfg(test)]
pub(crate) use model::{ArenaVisuals, Enemy, Hero, HeroBrain, HeroCombat, Loot};
pub(crate) use model::{CombatLog, GameStats, ProgressionNotice};
use presentation::PresentationPlugin;

pub fn run() -> Result<(), String> {
    let content =
        ContentCatalog::load().map_err(|error| format!("failed to load game content: {error}"))?;
    let (progression, progression_notice, profile_persistence) =
        match PlayerProgression::load(&content) {
            Ok(progression) => match validate_profile_items(&progression, &content) {
                Ok(()) => {
                    let persistence = ProfilePersistence::loaded(&progression);
                    (
                        progression,
                        "LOADED · Saved profile ready".to_string(),
                        persistence,
                    )
                }
                Err(error) => (
                    PlayerProgression::default(),
                    format!("PROFILE ERROR · Using a new in-memory profile · {error}"),
                    ProfilePersistence::error(),
                ),
            },
            Err(ProfileLoadError::NotFound(_)) => (
                PlayerProgression::default(),
                "NEW PROFILE · Press S to create the save file".to_string(),
                ProfilePersistence::missing(),
            ),
            Err(error) => (
                PlayerProgression::default(),
                format!("PROFILE ERROR · Using a new in-memory profile · {error}"),
                ProfilePersistence::error(),
            ),
        };

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.018, 0.022, 0.035)))
        .insert_resource(GameBalance::default())
        .insert_resource(GameStats::default())
        .insert_resource(CombatLog::default())
        .insert_resource(progression)
        .insert_resource(profile_persistence)
        .insert_resource(ProgressionNotice(progression_notice))
        .insert_resource(content)
        .insert_resource(ScriptRuntime::new())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Scripted ARPG MVP".into(),
                resolution: WindowResolution::new(1440, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((GameplayPlugin, ControlsPlugin, PresentationPlugin))
        .run();
    Ok(())
}
