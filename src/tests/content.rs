use std::fs;

use crate::{
    content::{CONTENT_PATH, ContentCatalog},
    items::{EquipmentSlot, LootRarity},
};

#[test]
fn shipped_content_catalog_is_valid_and_resolvable() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let encounter = content.starting_encounter();
    let spawn_count = encounter
        .spawn_groups
        .iter()
        .map(|id| {
            content
                .spawn_group(id)
                .expect("encounter group should resolve")
                .enemies
                .len()
        })
        .sum::<usize>();

    assert_eq!(encounter.id, "arena_one");
    assert_eq!(spawn_count, 8);
    assert_eq!(
        content
            .item("warded_plate")
            .expect("equipment should resolve")
            .rarity,
        LootRarity::Epic
    );
    assert_eq!(
        content
            .item("warded_plate")
            .expect("equipment should resolve")
            .slot,
        EquipmentSlot::Armor
    );
}

#[test]
fn unknown_item_drops_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("drop_item: \"scout_boots\"", "drop_item: \"missing\"", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown item drop should fail validation");

    assert!(error.contains("references unknown item `missing`"));
}

#[test]
fn unknown_archetype_references_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("archetype: \"grunt\"", "archetype: \"missing\"", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown archetype reference should fail validation");

    assert!(error.contains("unknown enemy archetype `missing`"));
}

#[test]
fn unknown_spawn_group_references_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen(
        "spawn_groups: [\"outer_ring\"]",
        "spawn_groups: [\"missing\"]",
        1,
    );
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown spawn group reference should fail validation");

    assert!(error.contains("unknown spawn group `missing`"));
}
