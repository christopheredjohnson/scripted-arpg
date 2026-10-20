use crate::{
    items::{Equipment, EquipmentSlot, Inventory, ItemDefinition, LootRarity},
    stats::{StatModifier, StatModifiers},
};

fn item(
    id: &str,
    name: &str,
    rarity: LootRarity,
    slot: EquipmentSlot,
    modifiers: StatModifiers,
) -> ItemDefinition {
    ItemDefinition {
        id: id.to_string(),
        name: name.to_string(),
        rarity,
        slot,
        modifiers,
    }
}

#[test]
fn inventory_preserves_each_collected_item() {
    let mut inventory = Inventory::default();
    inventory.add("scout_boots");
    inventory.add("scout_boots");

    assert_eq!(inventory.len(), 2);
}

#[test]
fn equipment_combines_modifiers_from_different_slots() {
    let weapon = item(
        "blade",
        "Blade",
        LootRarity::Rare,
        EquipmentSlot::Weapon,
        StatModifiers {
            damage: StatModifier {
                flat: 5.0,
                increased: 0.1,
            },
            ..Default::default()
        },
    );
    let armor = item(
        "plate",
        "Plate",
        LootRarity::Epic,
        EquipmentSlot::Armor,
        StatModifiers {
            defense: StatModifier {
                flat: 20.0,
                increased: 0.0,
            },
            ..Default::default()
        },
    );
    let mut equipment = Equipment::default();

    assert!(equipment.equip_if_upgrade(&weapon));
    assert!(equipment.equip_if_upgrade(&armor));
    assert_eq!(equipment.modifiers().damage.flat, 5.0);
    assert_eq!(equipment.modifiers().damage.increased, 0.1);
    assert_eq!(equipment.modifiers().defense.flat, 20.0);
}

#[test]
fn auto_equip_does_not_replace_an_item_with_lower_rarity() {
    let rare = item(
        "rare_blade",
        "Rare Blade",
        LootRarity::Rare,
        EquipmentSlot::Weapon,
        Default::default(),
    );
    let common = item(
        "common_blade",
        "Common Blade",
        LootRarity::Common,
        EquipmentSlot::Weapon,
        Default::default(),
    );
    let mut equipment = Equipment::default();

    assert!(equipment.equip_if_upgrade(&rare));
    assert!(!equipment.equip_if_upgrade(&common));
    assert_eq!(
        equipment.item_name(EquipmentSlot::Weapon),
        Some("Rare Blade")
    );
}
