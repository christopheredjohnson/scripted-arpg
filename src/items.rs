use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::stats::StatModifiers;

pub const INVENTORY_CAPACITY: usize = 24;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum LootRarity {
    Common,
    Rare,
    Epic,
}

impl LootRarity {
    pub fn score(self) -> u32 {
        match self {
            Self::Common => 10,
            Self::Rare => 35,
            Self::Epic => 100,
        }
    }

    pub fn affix_count(self) -> usize {
        match self {
            Self::Common => 0,
            Self::Rare => 1,
            Self::Epic => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Boots,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinition {
    pub id: String,
    pub name: String,
    pub rarity: LootRarity,
    pub slot: EquipmentSlot,
    pub modifiers: StatModifiers,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemInstance {
    pub instance_id: String,
    pub base_id: String,
    pub name: String,
    pub rarity: LootRarity,
    pub slot: EquipmentSlot,
    pub item_level: u32,
    pub affixes: Vec<String>,
    pub modifiers: StatModifiers,
}

impl ItemInstance {
    pub fn from_definition(
        instance_id: impl Into<String>,
        definition: &ItemDefinition,
        item_level: u32,
        affixes: Vec<String>,
        affix_names: &[String],
        modifiers: StatModifiers,
    ) -> Self {
        let name = if affix_names.is_empty() {
            definition.name.clone()
        } else {
            format!("{} {}", affix_names.join(" "), definition.name)
        };
        Self {
            instance_id: instance_id.into(),
            base_id: definition.id.clone(),
            name,
            rarity: definition.rarity,
            slot: definition.slot,
            item_level: item_level.max(1),
            affixes,
            modifiers,
        }
    }

    pub fn legacy(index: usize, definition: &ItemDefinition) -> Self {
        Self::from_definition(
            format!("legacy-{index}-{}", definition.id),
            definition,
            1,
            Vec::new(),
            &[],
            definition.modifiers,
        )
    }

    pub fn power_score(&self) -> f32 {
        let modifier_score =
            |flat: f32, increased: f32, flat_weight: f32| flat * flat_weight + increased * 100.0;
        modifier_score(
            self.modifiers.max_health.flat,
            self.modifiers.max_health.increased,
            0.1,
        ) + modifier_score(
            self.modifiers.damage.flat,
            self.modifiers.damage.increased,
            1.0,
        ) + modifier_score(
            self.modifiers.defense.flat,
            self.modifiers.defense.increased,
            0.3,
        ) + modifier_score(
            self.modifiers.move_speed.flat,
            self.modifiers.move_speed.increased,
            4.0,
        ) + modifier_score(
            self.modifiers.attack_speed.flat,
            self.modifiers.attack_speed.increased,
            30.0,
        )
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Inventory {
    items: Vec<ItemInstance>,
}

impl Inventory {
    pub fn from_items(items: Vec<ItemInstance>) -> Self {
        Self { items }
    }

    pub fn add(&mut self, item: ItemInstance) -> bool {
        if self.is_full() {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_full(&self) -> bool {
        self.items.len() >= INVENTORY_CAPACITY
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EquippedItem {
    instance_id: String,
    name: String,
    power_score: f32,
    modifiers: StatModifiers,
}

impl From<&ItemInstance> for EquippedItem {
    fn from(item: &ItemInstance) -> Self {
        Self {
            instance_id: item.instance_id.clone(),
            name: item.name.clone(),
            power_score: item.power_score(),
            modifiers: item.modifiers,
        }
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Equipment {
    weapon: Option<EquippedItem>,
    armor: Option<EquippedItem>,
    boots: Option<EquippedItem>,
}

impl Equipment {
    pub fn equip(&mut self, item: &ItemInstance) -> Option<String> {
        let equipped = EquippedItem::from(item);
        self.slot_mut(item.slot)
            .replace(equipped)
            .map(|previous| previous.instance_id)
    }

    pub fn equip_if_upgrade(&mut self, item: &ItemInstance) -> bool {
        if self
            .slot(item.slot)
            .as_ref()
            .is_some_and(|equipped| equipped.power_score >= item.power_score())
        {
            return false;
        }
        self.equip(item);
        true
    }

    pub fn item_name(&self, slot: EquipmentSlot) -> Option<&str> {
        self.slot(slot).as_ref().map(|item| item.name.as_str())
    }

    pub fn modifiers(&self) -> StatModifiers {
        [
            self.weapon.as_ref(),
            self.armor.as_ref(),
            self.boots.as_ref(),
        ]
        .into_iter()
        .flatten()
        .fold(StatModifiers::default(), |total, item| {
            total + item.modifiers
        })
    }

    fn slot(&self, slot: EquipmentSlot) -> &Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &self.weapon,
            EquipmentSlot::Armor => &self.armor,
            EquipmentSlot::Boots => &self.boots,
        }
    }

    fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Boots => &mut self.boots,
        }
    }
}
