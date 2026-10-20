use std::{collections::HashSet, fs, path::PathBuf};

use bevy::prelude::Resource;
use serde::Deserialize;

use crate::items::{EquipmentSlot, ItemDefinition, ItemInstance};
use crate::stats::{ActorStats, BaseStats, StatModifier, StatModifiers};
use crate::status_effects::StatusEffectSpec;

pub const CONTENT_PATH: &str = "assets/content.ron";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum AbilityTarget {
    NearestEnemy,
    Area {
        #[serde(default)]
        max_targets: Option<usize>,
        #[serde(default)]
        approach: bool,
    },
    SelfCast,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityDefinition {
    pub id: String,
    pub name: String,
    pub target: AbilityTarget,
    #[serde(default)]
    pub base_damage: f32,
    #[serde(default)]
    pub damage_scaling: f32,
    pub range: f32,
    pub cooldown: f32,
    #[serde(default = "default_move_speed_multiplier")]
    pub move_speed_multiplier: f32,
    #[serde(default)]
    pub target_effects: Vec<StatusEffectSpec>,
    #[serde(default)]
    pub self_effects: Vec<StatusEffectSpec>,
}

fn default_move_speed_multiplier() -> f32 {
    1.0
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassDefinition {
    pub id: String,
    pub name: String,
    pub abilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyArchetype {
    pub id: String,
    pub stats: BaseStats,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    #[serde(default = "default_attack_windup")]
    pub attack_windup: f32,
    pub drop_table: String,
    pub xp_reward: u32,
    #[serde(default)]
    pub on_hit_effect: Option<StatusEffectSpec>,
}

fn default_attack_windup() -> f32 {
    0.4
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropEntry {
    pub item: String,
    pub weight: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropTable {
    pub id: String,
    pub entries: Vec<DropEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffixDefinition {
    pub id: String,
    pub name: String,
    pub slots: Vec<EquipmentSlot>,
    pub min_item_level: u32,
    pub weight: u32,
    pub modifiers: StatModifiers,
}

impl EnemyArchetype {
    pub fn actor_stats_for_wave(&self, scaling: WaveScaling, wave_index: usize) -> ActorStats {
        ActorStats::new(self.stats).with_modifiers(scaling.enemy_modifiers(wave_index))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveScaling {
    pub health_per_wave: f32,
    pub damage_per_wave: f32,
    pub defense_per_wave: f32,
    pub move_speed_per_wave: f32,
    pub attack_speed_per_wave: f32,
    pub xp_per_wave: f32,
}

impl WaveScaling {
    fn enemy_modifiers(self, wave_index: usize) -> StatModifiers {
        let wave = wave_index as f32;
        StatModifiers {
            max_health: StatModifier {
                increased: self.health_per_wave * wave,
                ..Default::default()
            },
            damage: StatModifier {
                increased: self.damage_per_wave * wave,
                ..Default::default()
            },
            defense: StatModifier {
                flat: self.defense_per_wave * wave,
                ..Default::default()
            },
            move_speed: StatModifier {
                increased: self.move_speed_per_wave * wave,
                ..Default::default()
            },
            attack_speed: StatModifier {
                increased: self.attack_speed_per_wave * wave,
                ..Default::default()
            },
        }
    }

    pub fn scale_xp(self, base_xp: u32, wave_index: usize) -> u32 {
        let multiplier = 1.0 + self.xp_per_wave * wave_index as f32;
        ((base_xp as f32 * multiplier).round() as u32).max(1)
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
    pub default_class: String,
    pub classes: Vec<ClassDefinition>,
    pub abilities: Vec<AbilityDefinition>,
    pub encounter_sequence: Vec<String>,
    pub wave_scaling: WaveScaling,
    pub items: Vec<ItemDefinition>,
    pub affixes: Vec<AffixDefinition>,
    pub drop_tables: Vec<DropTable>,
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

    pub fn encounter_at(&self, index: usize) -> Option<&EncounterDefinition> {
        self.encounter_sequence
            .get(index)
            .and_then(|id| self.encounter(id))
    }

    pub fn class(&self, id: &str) -> Option<&ClassDefinition> {
        self.classes.iter().find(|class| class.id == id)
    }

    pub fn ability(&self, id: &str) -> Option<&AbilityDefinition> {
        self.abilities.iter().find(|ability| ability.id == id)
    }

    pub fn encounter_for_wave(&self, wave_index: usize) -> Option<&EncounterDefinition> {
        let sequence_index = wave_index.checked_rem(self.encounter_sequence.len())?;
        self.encounter_at(sequence_index)
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

    pub fn affix(&self, id: &str) -> Option<&AffixDefinition> {
        self.affixes.iter().find(|affix| affix.id == id)
    }

    pub fn drop_table(&self, id: &str) -> Option<&DropTable> {
        self.drop_tables.iter().find(|table| table.id == id)
    }

    pub fn generate_drop(
        &self,
        table_id: &str,
        item_level: u32,
        seed: u64,
    ) -> Result<ItemInstance, String> {
        let table = self
            .drop_table(table_id)
            .ok_or_else(|| format!("unknown drop table `{table_id}`"))?;
        let entry_index = weighted_index(
            table.entries.iter().map(|entry| entry.weight),
            mixed_seed(seed),
        )
        .ok_or_else(|| format!("drop table `{table_id}` has no weighted entries"))?;
        let definition = self
            .item(&table.entries[entry_index].item)
            .expect("validated drop table item");
        let item_level = item_level.max(1);
        let mut candidates: Vec<_> = self
            .affixes
            .iter()
            .filter(|affix| {
                affix.min_item_level <= item_level && affix.slots.contains(&definition.slot)
            })
            .collect();
        let mut selected = Vec::new();
        for affix_roll in 0..definition.rarity.affix_count() {
            let Some(index) = weighted_index(
                candidates.iter().map(|affix| affix.weight),
                mixed_seed(seed.wrapping_add(affix_roll as u64 + 1)),
            ) else {
                break;
            };
            selected.push(candidates.remove(index));
        }

        let level_multiplier = 1.0 + (item_level - 1) as f32 * 0.04;
        let modifiers = selected.iter().fold(
            definition.modifiers.scaled(level_multiplier),
            |total, affix| total + affix.modifiers.scaled(level_multiplier),
        );
        let affix_ids = selected.iter().map(|affix| affix.id.clone()).collect();
        let affix_names: Vec<_> = selected.iter().map(|affix| affix.name.clone()).collect();
        Ok(ItemInstance::from_definition(
            format!("{}-{item_level}-{seed:016x}", definition.id),
            definition,
            item_level,
            affix_ids,
            &affix_names,
            modifiers,
        ))
    }

    pub fn validate_item_instance(&self, item: &ItemInstance) -> Result<(), String> {
        let definition = self
            .item(&item.base_id)
            .ok_or_else(|| format!("inventory references unknown item `{}`", item.base_id))?;
        if item.instance_id.trim().is_empty() || item.item_level == 0 {
            return Err(format!(
                "item `{}` has invalid identity or level",
                item.base_id
            ));
        }
        if item.slot != definition.slot || item.rarity != definition.rarity {
            return Err(format!(
                "item `{}` does not match its definition",
                item.base_id
            ));
        }
        if item.affixes.len() > item.rarity.affix_count() {
            return Err(format!("item `{}` has too many affixes", item.base_id));
        }
        let mut seen_affixes = HashSet::new();
        let mut resolved_affixes = Vec::new();
        for affix_id in &item.affixes {
            if !seen_affixes.insert(affix_id) {
                return Err(format!("item has duplicate affix `{affix_id}`"));
            }
            let affix = self
                .affix(affix_id)
                .ok_or_else(|| format!("item references unknown affix `{affix_id}`"))?;
            if affix.min_item_level > item.item_level || !affix.slots.contains(&item.slot) {
                return Err(format!(
                    "affix `{affix_id}` is not valid for `{}`",
                    item.base_id
                ));
            }
            resolved_affixes.push(affix);
        }
        validate_modifiers(&item.base_id, item.modifiers)?;
        let level_multiplier = 1.0 + (item.item_level - 1) as f32 * 0.04;
        let expected_modifiers = resolved_affixes.iter().fold(
            definition.modifiers.scaled(level_multiplier),
            |total, affix| total + affix.modifiers.scaled(level_multiplier),
        );
        let expected_name = if resolved_affixes.is_empty() {
            definition.name.clone()
        } else {
            format!(
                "{} {}",
                resolved_affixes
                    .iter()
                    .map(|affix| affix.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                definition.name
            )
        };
        if item.modifiers != expected_modifiers || item.name != expected_name {
            return Err(format!(
                "item `{}` has inconsistent rolled values",
                item.base_id
            ));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        validate_unique_ids("class", self.classes.iter().map(|class| class.id.as_str()))?;
        validate_unique_ids(
            "ability",
            self.abilities.iter().map(|ability| ability.id.as_str()),
        )?;
        validate_unique_ids("item", self.items.iter().map(|item| item.id.as_str()))?;
        validate_unique_ids("affix", self.affixes.iter().map(|affix| affix.id.as_str()))?;
        validate_unique_ids(
            "drop table",
            self.drop_tables.iter().map(|table| table.id.as_str()),
        )?;
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

        if self.class(&self.default_class).is_none() {
            return Err(format!(
                "default class references unknown class `{}`",
                self.default_class
            ));
        }
        for ability in &self.abilities {
            validate_ability(ability)?;
        }
        for class in &self.classes {
            if class.name.trim().is_empty() || class.abilities.is_empty() {
                return Err(format!("class `{}` has no name or abilities", class.id));
            }
            let mut seen = HashSet::new();
            for ability_id in &class.abilities {
                if !seen.insert(ability_id) {
                    return Err(format!(
                        "class `{}` contains duplicate ability `{ability_id}`",
                        class.id
                    ));
                }
                if self.ability(ability_id).is_none() {
                    return Err(format!(
                        "class `{}` references unknown ability `{ability_id}`",
                        class.id
                    ));
                }
            }
        }

        if self.encounter_sequence.is_empty() {
            return Err("encounter sequence cannot be empty".to_string());
        }
        validate_unique_ids(
            "encounter sequence entry",
            self.encounter_sequence.iter().map(String::as_str),
        )?;
        for encounter_id in &self.encounter_sequence {
            if self.encounter(encounter_id).is_none() {
                return Err(format!(
                    "encounter sequence references unknown encounter `{encounter_id}`"
                ));
            }
        }

        let scaling_values = [
            self.wave_scaling.health_per_wave,
            self.wave_scaling.damage_per_wave,
            self.wave_scaling.defense_per_wave,
            self.wave_scaling.move_speed_per_wave,
            self.wave_scaling.attack_speed_per_wave,
            self.wave_scaling.xp_per_wave,
        ];
        if scaling_values
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        {
            return Err("wave scaling values must be finite and non-negative".to_string());
        }

        for item in &self.items {
            validate_item(item)?;
        }
        for affix in &self.affixes {
            if affix.name.trim().is_empty()
                || affix.slots.is_empty()
                || affix.min_item_level == 0
                || affix.weight == 0
            {
                return Err(format!(
                    "affix `{}` has invalid generation values",
                    affix.id
                ));
            }
            validate_modifiers(&affix.id, affix.modifiers)?;
        }
        for table in &self.drop_tables {
            if table.entries.is_empty() {
                return Err(format!("drop table `{}` has no entries", table.id));
            }
            for entry in &table.entries {
                if entry.weight == 0 || self.item(&entry.item).is_none() {
                    return Err(format!(
                        "drop table `{}` has an invalid item entry `{}`",
                        table.id, entry.item
                    ));
                }
            }
        }
        for archetype in &self.enemy_archetypes {
            validate_archetype(archetype)?;
            if let Some(effect) = &archetype.on_hit_effect {
                effect.validate().map_err(|error| {
                    format!(
                        "enemy archetype `{}` has invalid on-hit effect: {error}",
                        archetype.id
                    )
                })?;
            }
            if self.drop_table(&archetype.drop_table).is_none() {
                return Err(format!(
                    "enemy archetype `{}` references unknown drop table `{}`",
                    archetype.id, archetype.drop_table
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

fn validate_ability(ability: &AbilityDefinition) -> Result<(), String> {
    let valid_target = match ability.target {
        AbilityTarget::SelfCast => ability.range >= 0.0,
        AbilityTarget::NearestEnemy => ability.range > 0.0,
        AbilityTarget::Area { max_targets, .. } => {
            ability.range > 0.0 && max_targets.is_none_or(|count| count > 0)
        }
    };
    let valid = !ability.name.trim().is_empty()
        && ability.base_damage.is_finite()
        && ability.base_damage >= 0.0
        && ability.damage_scaling.is_finite()
        && ability.damage_scaling >= 0.0
        && ability.range.is_finite()
        && ability.cooldown.is_finite()
        && ability.cooldown >= 0.0
        && ability.move_speed_multiplier.is_finite()
        && ability.move_speed_multiplier > 0.0
        && valid_target;
    if !valid {
        return Err(format!(
            "ability `{}` has invalid combat values",
            ability.id
        ));
    }
    for effect in ability.target_effects.iter().chain(&ability.self_effects) {
        effect
            .validate()
            .map_err(|error| format!("ability `{}` has invalid effect: {error}", ability.id))?;
    }
    Ok(())
}

fn validate_item(item: &ItemDefinition) -> Result<(), String> {
    if item.name.trim().is_empty() {
        return Err(format!("item `{}` has an empty name", item.id));
    }
    validate_modifiers(&item.id, item.modifiers)
}

fn validate_modifiers(id: &str, modifiers: StatModifiers) -> Result<(), String> {
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
    if values
        .into_iter()
        .any(|value| !value.is_finite() || value < 0.0)
    {
        return Err(format!(
            "item `{}` has invalid stat modifiers; item bonuses must be finite and non-negative",
            id
        ));
    }
    Ok(())
}

fn weighted_index(weights: impl Iterator<Item = u32>, seed: u64) -> Option<usize> {
    let weights: Vec<_> = weights.collect();
    let total: u64 = weights.iter().map(|weight| u64::from(*weight)).sum();
    if total == 0 {
        return None;
    }
    let mut roll = seed % total;
    for (index, weight) in weights.into_iter().enumerate() {
        if roll < u64::from(weight) {
            return Some(index);
        }
        roll -= u64::from(weight);
    }
    None
}

fn mixed_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
        && archetype.attack_windup.is_finite()
        && archetype.attack_windup > 0.0
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
