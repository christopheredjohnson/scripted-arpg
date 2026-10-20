use std::{fs, path::PathBuf};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::stats::{StatModifier, StatModifiers};

pub const SAVE_PATH: &str = "saves/profile.ron";
const SAVE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeKind {
    Power,
    Vitality,
    Swiftness,
}

impl UpgradeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Power => "Power",
            Self::Vitality => "Vitality",
            Self::Swiftness => "Swiftness",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeRanks {
    power: u32,
    vitality: u32,
    swiftness: u32,
}

impl UpgradeRanks {
    pub fn power(self) -> u32 {
        self.power
    }

    pub fn vitality(self) -> u32 {
        self.vitality
    }

    pub fn swiftness(self) -> u32 {
        self.swiftness
    }

    fn total(self) -> u32 {
        self.power
            .saturating_add(self.vitality)
            .saturating_add(self.swiftness)
    }

    fn increment(&mut self, kind: UpgradeKind) {
        match kind {
            UpgradeKind::Power => self.power += 1,
            UpgradeKind::Vitality => self.vitality += 1,
            UpgradeKind::Swiftness => self.swiftness += 1,
        }
    }
}

#[derive(Resource, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerProgression {
    level: u32,
    experience: u32,
    unspent_points: u32,
    upgrades: UpgradeRanks,
}

impl Default for PlayerProgression {
    fn default() -> Self {
        Self {
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: UpgradeRanks::default(),
        }
    }
}

impl PlayerProgression {
    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn experience(&self) -> u32 {
        self.experience
    }

    pub fn experience_to_next_level(&self) -> u32 {
        experience_to_next_level(self.level)
    }

    pub fn unspent_points(&self) -> u32 {
        self.unspent_points
    }

    pub fn upgrades(&self) -> UpgradeRanks {
        self.upgrades
    }

    pub fn award_experience(&mut self, amount: u32) -> u32 {
        self.experience = self.experience.saturating_add(amount);
        let mut levels_gained = 0;
        while self.experience >= self.experience_to_next_level() {
            self.experience -= self.experience_to_next_level();
            self.level = self.level.saturating_add(1);
            self.unspent_points = self.unspent_points.saturating_add(1);
            levels_gained += 1;
        }
        levels_gained
    }

    pub fn spend_point(&mut self, kind: UpgradeKind) -> bool {
        if self.unspent_points == 0 {
            return false;
        }
        self.unspent_points -= 1;
        self.upgrades.increment(kind);
        true
    }

    pub fn modifiers(&self) -> StatModifiers {
        StatModifiers {
            max_health: StatModifier {
                flat: self.upgrades.vitality as f32 * 15.0,
                increased: 0.0,
            },
            damage: StatModifier {
                flat: self.upgrades.power as f32 * 2.0,
                increased: 0.0,
            },
            move_speed: StatModifier {
                flat: self.upgrades.swiftness as f32 * 0.2,
                increased: 0.0,
            },
            attack_speed: StatModifier {
                flat: 0.0,
                increased: self.upgrades.swiftness as f32 * 0.05,
            },
            ..Default::default()
        }
    }

    pub fn load() -> Result<Self, String> {
        let path = save_path();
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        Self::from_ron(&source).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn save(&self) -> Result<(), String> {
        let path = save_path();
        let parent = path
            .parent()
            .ok_or_else(|| "save path has no parent directory".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        let source = self.to_ron()?;
        let temporary = path.with_extension("ron.tmp");
        fs::write(&temporary, source)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))
    }

    pub fn from_ron(source: &str) -> Result<Self, String> {
        let save: SaveData = ron::from_str(source).map_err(|error| error.to_string())?;
        if save.version != SAVE_VERSION {
            return Err(format!(
                "unsupported save version {}; expected {SAVE_VERSION}",
                save.version
            ));
        }
        save.progression.validate()?;
        Ok(save.progression)
    }

    pub fn to_ron(&self) -> Result<String, String> {
        self.validate()?;
        ron::ser::to_string_pretty(
            &SaveData {
                version: SAVE_VERSION,
                progression: self.clone(),
            },
            ron::ser::PrettyConfig::default(),
        )
        .map_err(|error| error.to_string())
    }

    fn validate(&self) -> Result<(), String> {
        if self.level == 0 {
            return Err("player level must be at least 1".to_string());
        }
        if self.experience >= self.experience_to_next_level() {
            return Err("stored experience must be below the next level threshold".to_string());
        }
        let earned_points = self.level - 1;
        if self.upgrades.total().saturating_add(self.unspent_points) != earned_points {
            return Err("upgrade points do not match the stored player level".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SaveData {
    version: u32,
    progression: PlayerProgression,
}

fn experience_to_next_level(level: u32) -> u32 {
    50_u32.saturating_add(level.saturating_sub(1).saturating_mul(25))
}

fn save_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SAVE_PATH)
}
