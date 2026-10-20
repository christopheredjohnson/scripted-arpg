use bevy::prelude::Component;
use serde::Deserialize;

use crate::stats::StatModifiers;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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

    fn rank(self) -> u8 {
        match self {
            Self::Common => 0,
            Self::Rare => 1,
            Self::Epic => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Boots,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinition {
    pub id: String,
    pub name: String,
    pub rarity: LootRarity,
    pub slot: EquipmentSlot,
    pub modifiers: StatModifiers,
}

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    items: Vec<String>,
}

impl Inventory {
    pub fn add(&mut self, item_id: impl Into<String>) {
        self.items.push(item_id.into());
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EquippedItem {
    id: String,
    name: String,
    rarity: LootRarity,
    modifiers: StatModifiers,
}

impl From<&ItemDefinition> for EquippedItem {
    fn from(item: &ItemDefinition) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            rarity: item.rarity,
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
    pub fn equip(&mut self, item: &ItemDefinition) -> Option<String> {
        let equipped = EquippedItem::from(item);
        self.slot_mut(item.slot)
            .replace(equipped)
            .map(|previous| previous.id)
    }

    pub fn equip_if_upgrade(&mut self, item: &ItemDefinition) -> bool {
        let slot = self.slot_mut(item.slot);
        if slot
            .as_ref()
            .is_some_and(|equipped| equipped.rarity.rank() >= item.rarity.rank())
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
