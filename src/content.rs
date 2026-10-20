use std::{collections::HashSet, fs, path::PathBuf};

use bevy::prelude::Resource;
use serde::Deserialize;

use crate::items::ItemDefinition;
use crate::stats::{ActorStats, BaseStats};

pub const CONTENT_PATH: &str = "assets/content.ron";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyArchetype {
    pub id: String,
    pub stats: BaseStats,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub drop_item: String,
    pub xp_reward: u32,
}

impl EnemyArchetype {
    pub fn actor_stats(&self) -> ActorStats {
        ActorStats::new(self.stats).with_modifiers(Default::default())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemySpawn {
    pub archetype: String,
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnGroup {
    pub id: String,
    pub attack_stagger: f32,
    pub enemies: Vec<EnemySpawn>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterDefinition {
    pub id: String,
    pub spawn_groups: Vec<String>,
}

#[derive(Resource, Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentCatalog {
    pub starting_encounter: String,
    pub items: Vec<ItemDefinition>,
    pub enemy_archetypes: Vec<EnemyArchetype>,
    pub spawn_groups: Vec<SpawnGroup>,
    pub encounters: Vec<EncounterDefinition>,
}

impl ContentCatalog {
    pub fn load() -> Result<Self, String> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        Self::from_ron(&source).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn from_ron(source: &str) -> Result<Self, String> {
        let catalog: Self = ron::from_str(source).map_err(|error| error.to_string())?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn starting_encounter(&self) -> &EncounterDefinition {
        self.encounter(&self.starting_encounter)
            .expect("validated content must contain its starting encounter")
    }

    pub fn encounter(&self, id: &str) -> Option<&EncounterDefinition> {
        self.encounters.iter().find(|encounter| encounter.id == id)
    }

    pub fn spawn_group(&self, id: &str) -> Option<&SpawnGroup> {
        self.spawn_groups.iter().find(|group| group.id == id)
    }

    pub fn enemy_archetype(&self, id: &str) -> Option<&EnemyArchetype> {
        self.enemy_archetypes
            .iter()
            .find(|archetype| archetype.id == id)
    }

    pub fn item(&self, id: &str) -> Option<&ItemDefinition> {
        self.items.iter().find(|item| item.id == id)
    }

    fn validate(&self) -> Result<(), String> {
        validate_unique_ids("item", self.items.iter().map(|item| item.id.as_str()))?;
        validate_unique_ids(
            "enemy archetype",
            self.enemy_archetypes.iter().map(|item| item.id.as_str()),
        )?;
        validate_unique_ids(
            "spawn group",
            self.spawn_groups.iter().map(|item| item.id.as_str()),
        )?;
        validate_unique_ids(
            "encounter",
            self.encounters.iter().map(|item| item.id.as_str()),
        )?;

        if self.encounter(&self.starting_encounter).is_none() {
            return Err(format!(
                "starting encounter `{}` is not defined",
                self.starting_encounter
            ));
        }

        for item in &self.items {
            validate_item(item)?;
        }
        for archetype in &self.enemy_archetypes {
            validate_archetype(archetype)?;
            if self.item(&archetype.drop_item).is_none() {
                return Err(format!(
                    "enemy archetype `{}` references unknown item `{}`",
                    archetype.id, archetype.drop_item
                ));
            }
        }
        for group in &self.spawn_groups {
            if !group.attack_stagger.is_finite() || group.attack_stagger < 0.0 {
                return Err(format!(
                    "spawn group `{}` has an invalid attack_stagger",
                    group.id
                ));
            }
            for spawn in &group.enemies {
                if self.enemy_archetype(&spawn.archetype).is_none() {
                    return Err(format!(
                        "spawn group `{}` references unknown enemy archetype `{}`",
                        group.id, spawn.archetype
                    ));
                }
                if !spawn.x.is_finite() || !spawn.z.is_finite() {
                    return Err(format!(
                        "spawn group `{}` contains a non-finite position",
                        group.id
                    ));
                }
            }
        }
        for encounter in &self.encounters {
            if encounter.spawn_groups.is_empty() {
                return Err(format!("encounter `{}` has no spawn groups", encounter.id));
            }
            for group in &encounter.spawn_groups {
                if self.spawn_group(group).is_none() {
                    return Err(format!(
                        "encounter `{}` references unknown spawn group `{group}`",
                        encounter.id
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_item(item: &ItemDefinition) -> Result<(), String> {
    let modifiers = item.modifiers;
    let values = [
        modifiers.max_health.flat,
        modifiers.max_health.increased,
        modifiers.damage.flat,
        modifiers.damage.increased,
        modifiers.defense.flat,
        modifiers.defense.increased,
        modifiers.move_speed.flat,
        modifiers.move_speed.increased,
        modifiers.attack_speed.flat,
        modifiers.attack_speed.increased,
    ];
    if item.name.trim().is_empty() {
        return Err(format!("item `{}` has an empty name", item.id));
    }
    if values.into_iter().any(|value| !value.is_finite()) {
        return Err(format!("item `{}` has invalid stat modifiers", item.id));
    }
    Ok(())
}

fn validate_unique_ids<'a>(kind: &str, ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = HashSet::new();
    for id in ids {
        if id.trim().is_empty() {
            return Err(format!("{kind} id cannot be empty"));
        }
        if !seen.insert(id) {
            return Err(format!("duplicate {kind} id `{id}`"));
        }
    }
    Ok(())
}

fn validate_archetype(archetype: &EnemyArchetype) -> Result<(), String> {
    let stats = archetype.stats;
    let valid = stats.max_health.is_finite()
        && stats.max_health > 0.0
        && stats.damage.is_finite()
        && stats.damage >= 0.0
        && stats.defense.is_finite()
        && stats.defense >= 0.0
        && stats.move_speed.is_finite()
        && stats.move_speed > 0.0
        && stats.attack_speed.is_finite()
        && stats.attack_speed > 0.0
        && archetype.attack_range.is_finite()
        && archetype.attack_range > 0.0
        && archetype.attack_cooldown.is_finite()
        && archetype.attack_cooldown >= 0.0
        && archetype.xp_reward > 0;
    if valid {
        Ok(())
    } else {
        Err(format!(
            "enemy archetype `{}` contains invalid combat values",
            archetype.id
        ))
    }
}
