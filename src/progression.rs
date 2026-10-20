use std::{collections::HashSet, fmt, fs, io::ErrorKind, path::PathBuf};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::ContentCatalog;
use crate::items::{EquipmentSlot, INVENTORY_CAPACITY, ItemInstance};
use crate::stats::{StatModifier, StatModifiers};

pub const SAVE_PATH: &str = "saves/profile.ron";
const SAVE_VERSION: u32 = 4;
const DEFAULT_CLASS_ID: &str = "vanguard";

fn default_class_id() -> String {
    DEFAULT_CLASS_ID.to_string()
}

fn default_ability_loadout() -> Vec<String> {
    ["strike", "cleave", "whirlwind", "charge", "guard"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfileFileState {
    Missing,
    Ready,
    Error,
}

#[derive(Resource, Clone, Debug)]
pub struct ProfilePersistence {
    persisted: Option<PlayerProgression>,
    file_state: ProfileFileState,
}

impl ProfilePersistence {
    pub fn loaded(progression: &PlayerProgression) -> Self {
        Self {
            persisted: Some(progression.clone()),
            file_state: ProfileFileState::Ready,
        }
    }

    pub fn missing() -> Self {
        Self {
            persisted: None,
            file_state: ProfileFileState::Missing,
        }
    }

    pub fn error() -> Self {
        Self {
            persisted: None,
            file_state: ProfileFileState::Error,
        }
    }

    pub fn mark_saved(&mut self, progression: &PlayerProgression) {
        self.persisted = Some(progression.clone());
        self.file_state = ProfileFileState::Ready;
    }

    pub fn mark_missing(&mut self) {
        self.persisted = None;
        self.file_state = ProfileFileState::Missing;
    }

    pub fn mark_error(&mut self) {
        self.file_state = ProfileFileState::Error;
    }

    pub fn has_unsaved_changes(&self, progression: &PlayerProgression) -> bool {
        self.persisted.as_ref() != Some(progression)
    }

    pub fn load_requires_confirmation(&self, progression: &PlayerProgression) -> bool {
        self.file_state == ProfileFileState::Ready && self.has_unsaved_changes(progression)
    }

    pub fn status_label(&self, progression: &PlayerProgression) -> &'static str {
        match self.file_state {
            ProfileFileState::Error => "FILE ERROR",
            ProfileFileState::Missing => "NOT SAVED",
            ProfileFileState::Ready if self.has_unsaved_changes(progression) => "UNSAVED CHANGES",
            ProfileFileState::Ready => "SAVED",
        }
    }
}

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

#[derive(Resource, Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerProgression {
    #[serde(default = "default_class_id")]
    class_id: String,
    #[serde(default = "default_ability_loadout")]
    ability_loadout: Vec<String>,
    level: u32,
    experience: u32,
    unspent_points: u32,
    upgrades: UpgradeRanks,
    #[serde(default)]
    inventory: Vec<ItemInstance>,
    #[serde(default)]
    equipment: SavedEquipment,
}

impl Default for PlayerProgression {
    fn default() -> Self {
        Self {
            class_id: default_class_id(),
            ability_loadout: default_ability_loadout(),
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: UpgradeRanks::default(),
            inventory: Vec::new(),
            equipment: SavedEquipment::default(),
        }
    }
}

impl PlayerProgression {
    pub fn class_id(&self) -> &str {
        &self.class_id
    }

    pub fn ability_loadout(&self) -> &[String] {
        &self.ability_loadout
    }

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

    pub fn items(&self) -> &[ItemInstance] {
        &self.inventory
    }

    pub fn equipped_instance(&self, slot: EquipmentSlot) -> Option<&str> {
        self.equipment.item(slot)
    }

    pub fn item(&self, instance_id: &str) -> Option<&ItemInstance> {
        self.inventory
            .iter()
            .find(|item| item.instance_id == instance_id)
    }

    pub fn record_item(&mut self, item: ItemInstance) -> bool {
        if self.inventory.len() >= INVENTORY_CAPACITY {
            return false;
        }
        self.inventory.push(item);
        true
    }

    pub fn set_equipped_item(&mut self, slot: EquipmentSlot, instance_id: impl Into<String>) {
        self.equipment.set(slot, instance_id.into());
    }

    pub fn validate_items(
        &self,
        mut slot_for_item: impl FnMut(&ItemInstance) -> Result<EquipmentSlot, String>,
    ) -> Result<(), String> {
        let mut instance_ids = HashSet::new();
        for item in &self.inventory {
            if !instance_ids.insert(item.instance_id.as_str()) {
                return Err(format!("duplicate item instance `{}`", item.instance_id));
            }
            slot_for_item(item)?;
        }
        for slot in [
            EquipmentSlot::Weapon,
            EquipmentSlot::Armor,
            EquipmentSlot::Boots,
        ] {
            let Some(instance_id) = self.equipped_instance(slot) else {
                continue;
            };
            let item = self
                .item(instance_id)
                .ok_or_else(|| format!("equipped item instance `{instance_id}` is not owned"))?;
            let actual_slot = slot_for_item(item)?;
            if actual_slot != slot {
                return Err(format!(
                    "item instance `{instance_id}` is stored in the wrong equipment slot"
                ));
            }
        }
        Ok(())
    }

    pub fn validate_class_selection(&self, content: &ContentCatalog) -> Result<(), String> {
        let class = content
            .class(&self.class_id)
            .ok_or_else(|| format!("profile references unknown class `{}`", self.class_id))?;
        if self.ability_loadout.is_empty() {
            return Err("ability loadout cannot be empty".to_string());
        }
        let mut seen = HashSet::new();
        for ability_id in &self.ability_loadout {
            if !seen.insert(ability_id) {
                return Err(format!("ability loadout contains duplicate `{ability_id}`"));
            }
            if !class.abilities.contains(ability_id) {
                return Err(format!(
                    "ability `{ability_id}` does not belong to class `{}`",
                    self.class_id
                ));
            }
        }
        Ok(())
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

    pub fn load(content: &ContentCatalog) -> Result<Self, ProfileLoadError> {
        let path = save_path();
        let source = fs::read_to_string(&path).map_err(|error| match error.kind() {
            ErrorKind::NotFound => ProfileLoadError::NotFound(path.clone()),
            _ => ProfileLoadError::Io(format!("could not read {}: {error}", path.display())),
        })?;
        Self::from_ron(&source, content)
            .map_err(|error| ProfileLoadError::Invalid(format!("{}: {error}", path.display())))
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

    pub fn from_ron(source: &str, content: &ContentCatalog) -> Result<Self, String> {
        let version: SaveVersion = ron::from_str(source).map_err(|error| error.to_string())?;
        let progression = match version.version {
            SAVE_VERSION | 3 => {
                ron::from_str::<SaveData>(source)
                    .map_err(|error| error.to_string())?
                    .progression
            }
            1 | 2 => {
                let legacy: LegacySaveData =
                    ron::from_str(source).map_err(|error| error.to_string())?;
                migrate_legacy(legacy.progression, content)?
            }
            unsupported => {
                return Err(format!(
                    "unsupported save version {unsupported}; expected {SAVE_VERSION}"
                ));
            }
        };
        progression.validate()?;
        progression.validate_items(|item| {
            content.validate_item_instance(item)?;
            Ok(item.slot)
        })?;
        progression.validate_class_selection(content)?;
        Ok(progression)
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
        if self.inventory.len() > INVENTORY_CAPACITY {
            return Err(format!(
                "inventory exceeds its capacity of {INVENTORY_CAPACITY}"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedEquipment {
    weapon: Option<String>,
    armor: Option<String>,
    boots: Option<String>,
}

impl SavedEquipment {
    fn item(&self, slot: EquipmentSlot) -> Option<&str> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.as_deref(),
            EquipmentSlot::Armor => self.armor.as_deref(),
            EquipmentSlot::Boots => self.boots.as_deref(),
        }
    }

    fn set(&mut self, slot: EquipmentSlot, item_id: String) {
        *match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Boots => &mut self.boots,
        } = Some(item_id);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileLoadError {
    NotFound(PathBuf),
    Io(String),
    Invalid(String),
}

impl fmt::Display for ProfileLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(formatter, "no profile exists at {}", path.display()),
            Self::Io(error) | Self::Invalid(error) => formatter.write_str(error),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SaveData {
    version: u32,
    progression: PlayerProgression,
}

#[derive(Debug, Deserialize)]
struct SaveVersion {
    version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySaveData {
    #[allow(dead_code)]
    version: u32,
    progression: LegacyProgression,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProgression {
    level: u32,
    experience: u32,
    unspent_points: u32,
    upgrades: UpgradeRanks,
    #[serde(default)]
    inventory: Vec<String>,
    #[serde(default)]
    equipment: SavedEquipment,
}

fn migrate_legacy(
    legacy: LegacyProgression,
    content: &ContentCatalog,
) -> Result<PlayerProgression, String> {
    let inventory: Vec<_> = legacy
        .inventory
        .iter()
        .enumerate()
        .map(|(index, item_id)| {
            content
                .item(item_id)
                .map(|definition| ItemInstance::legacy(index, definition))
                .ok_or_else(|| format!("inventory references unknown item `{item_id}`"))
        })
        .collect::<Result<_, _>>()?;
    let migrate_equipped = |slot: EquipmentSlot| {
        legacy.equipment.item(slot).map_or(Ok(None), |base_id| {
            inventory
                .iter()
                .find(|item| item.base_id == base_id && item.slot == slot)
                .map(|item| Some(item.instance_id.clone()))
                .ok_or_else(|| format!("equipped item `{base_id}` is not owned"))
        })
    };
    let equipment = SavedEquipment {
        weapon: migrate_equipped(EquipmentSlot::Weapon)?,
        armor: migrate_equipped(EquipmentSlot::Armor)?,
        boots: migrate_equipped(EquipmentSlot::Boots)?,
    };
    Ok(PlayerProgression {
        class_id: default_class_id(),
        ability_loadout: default_ability_loadout(),
        level: legacy.level,
        experience: legacy.experience,
        unspent_points: legacy.unspent_points,
        upgrades: legacy.upgrades,
        inventory,
        equipment,
    })
}

fn experience_to_next_level(level: u32) -> u32 {
    50_u32.saturating_add(level.saturating_sub(1).saturating_mul(25))
}

fn save_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SAVE_PATH)
}
