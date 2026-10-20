use crate::{
    items::{
        Equipment, EquipmentSlot, INVENTORY_CAPACITY, Inventory, ItemDefinition, ItemInstance,
        LootRarity,
    },
    stats::{StatModifier, StatModifiers},
};

fn item(
    id: &str,
    name: &str,
    rarity: LootRarity,
    slot: EquipmentSlot,
    modifiers: StatModifiers,
) -> ItemInstance {
    let definition = ItemDefinition {
        id: id.to_string(),
        name: name.to_string(),
        rarity,
        slot,
        modifiers,
    };
    ItemInstance::from_definition(id, &definition, 1, Vec::new(), &[], modifiers)
}

#[test]
fn inventory_preserves_each_collected_item() {
    let mut inventory = Inventory::default();
    let boots = item(
        "boots-one",
        "Scout Boots",
        LootRarity::Common,
        EquipmentSlot::Boots,
        Default::default(),
    );
    let mut other_boots = boots.clone();
    other_boots.instance_id = "boots-two".to_string();
    assert!(inventory.add(boots));
    assert!(inventory.add(other_boots));

    assert_eq!(inventory.len(), 2);
}

#[test]
fn inventory_rejects_items_beyond_its_capacity() {
    let mut inventory = Inventory::default();
    for index in 0..INVENTORY_CAPACITY {
        let mut boots = item(
            "boots",
            "Scout Boots",
            LootRarity::Common,
            EquipmentSlot::Boots,
            Default::default(),
        );
        boots.instance_id = format!("boots-{index}");
        assert!(inventory.add(boots));
    }
    assert!(inventory.is_full());
    assert!(!inventory.add(item(
        "overflow",
        "Overflow Boots",
        LootRarity::Common,
        EquipmentSlot::Boots,
        Default::default(),
    )));
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
fn auto_equip_uses_rolled_stats_instead_of_rarity() {
    let weak_rare = item(
        "weak-rare",
        "Weak Rare Blade",
        LootRarity::Rare,
        EquipmentSlot::Weapon,
        StatModifiers {
            damage: StatModifier {
                flat: 1.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let strong_common = item(
        "strong-common",
        "Strong Common Blade",
        LootRarity::Common,
        EquipmentSlot::Weapon,
        StatModifiers {
            damage: StatModifier {
                flat: 8.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut equipment = Equipment::default();

    assert!(equipment.equip_if_upgrade(&weak_rare));
    assert!(equipment.equip_if_upgrade(&strong_common));
    assert_eq!(
        equipment.item_name(EquipmentSlot::Weapon),
        Some("Strong Common Blade")
    );
}
