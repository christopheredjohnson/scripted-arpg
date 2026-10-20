use crate::{
    content::ContentCatalog,
    items::{EquipmentSlot, ItemInstance},
    progression::{PlayerProgression, ProfilePersistence, UpgradeKind},
};

#[test]
fn experience_can_grant_multiple_levels_and_upgrade_points() {
    let mut progression = PlayerProgression::default();

    assert_eq!(progression.award_experience(125), 2);
    assert_eq!(progression.level(), 3);
    assert_eq!(progression.experience(), 0);
    assert_eq!(progression.unspent_points(), 2);
}

#[test]
fn spending_points_builds_permanent_stat_modifiers() {
    let mut progression = PlayerProgression::default();
    progression.award_experience(125);

    assert!(progression.spend_point(UpgradeKind::Power));
    assert!(progression.spend_point(UpgradeKind::Swiftness));
    assert!(!progression.spend_point(UpgradeKind::Vitality));

    let modifiers = progression.modifiers();
    assert_eq!(modifiers.damage.flat, 2.0);
    assert_eq!(modifiers.move_speed.flat, 0.2);
    assert_eq!(modifiers.attack_speed.increased, 0.05);
}

#[test]
fn progression_round_trips_through_the_versioned_save_format() {
    let content = ContentCatalog::load().expect("content should load");
    let mut progression = PlayerProgression::default();
    progression.award_experience(80);
    progression.spend_point(UpgradeKind::Vitality);

    let encoded = progression.to_ron().expect("profile should serialize");
    let decoded =
        PlayerProgression::from_ron(&encoded, &content).expect("profile should deserialize");

    assert_eq!(decoded, progression);
}

#[test]
fn inconsistent_progression_saves_are_rejected() {
    let content = ContentCatalog::load().expect("content should load");
    let encoded = PlayerProgression::default()
        .to_ron()
        .expect("default profile should serialize");
    let invalid = encoded.replacen("level: 1", "level: 3", 1);

    let error = PlayerProgression::from_ron(&invalid, &content)
        .expect_err("unearned or missing points should invalidate a save");
    assert!(error.contains("upgrade points"));
}

#[test]
fn inventory_and_equipment_round_trip_with_the_profile() {
    let content = ContentCatalog::load().expect("content should load");
    let mut profile = PlayerProgression::default();
    let item = ItemInstance::legacy(0, content.item("raider_blade").expect("blade should exist"));
    let instance_id = item.instance_id.clone();
    assert!(profile.record_item(item.clone()));
    profile.set_equipped_item(EquipmentSlot::Weapon, instance_id.clone());

    let encoded = profile.to_ron().expect("profile should serialize");
    let decoded =
        PlayerProgression::from_ron(&encoded, &content).expect("profile should deserialize");

    assert_eq!(decoded.items(), &[item]);
    assert_eq!(
        decoded.equipped_instance(EquipmentSlot::Weapon),
        Some(instance_id.as_str())
    );
    assert!(decoded.validate_items(|item| Ok(item.slot)).is_ok());
}

#[test]
fn version_one_progression_saves_migrate_with_empty_items() {
    let content = ContentCatalog::load().expect("content should load");
    let legacy = r#"(
        version: 1,
        progression: (
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: (power: 0, vitality: 0, swiftness: 0),
        ),
    )"#;

    let migrated =
        PlayerProgression::from_ron(legacy, &content).expect("v1 profile should migrate");
    assert!(migrated.items().is_empty());
    assert_eq!(migrated.equipped_instance(EquipmentSlot::Weapon), None);
}

#[test]
fn version_two_item_ids_migrate_to_level_one_instances() {
    let content = ContentCatalog::load().expect("content should load");
    let legacy = r#"(
        version: 2,
        progression: (
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: (power: 0, vitality: 0, swiftness: 0),
            inventory: ["raider_blade"],
            equipment: (weapon: Some("raider_blade"), armor: None, boots: None),
        ),
    )"#;

    let migrated =
        PlayerProgression::from_ron(legacy, &content).expect("v2 profile should migrate");
    assert_eq!(migrated.items()[0].base_id, "raider_blade");
    assert_eq!(migrated.items()[0].item_level, 1);
    assert_eq!(
        migrated.equipped_instance(EquipmentSlot::Weapon),
        Some(migrated.items()[0].instance_id.as_str())
    );
}

#[test]
fn version_three_profiles_migrate_to_the_vanguard_loadout() {
    let content = ContentCatalog::load().expect("content should load");
    let legacy = r#"(
        version: 3,
        progression: (
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: (power: 0, vitality: 0, swiftness: 0),
        ),
    )"#;

    let migrated =
        PlayerProgression::from_ron(legacy, &content).expect("v3 profile should migrate");
    assert_eq!(migrated.class_id(), "vanguard");
    assert_eq!(
        migrated.ability_loadout(),
        ["strike", "cleave", "whirlwind", "charge", "guard"]
    );
}

#[test]
fn profiles_reject_abilities_outside_the_selected_class() {
    let content = ContentCatalog::load().expect("content should load");
    let invalid = r#"(
        version: 4,
        progression: (
            class_id: "vanguard",
            ability_loadout: ["strike", "missing"],
            level: 1,
            experience: 0,
            unspent_points: 0,
            upgrades: (power: 0, vitality: 0, swiftness: 0),
        ),
    )"#;

    let error = PlayerProgression::from_ron(invalid, &content)
        .expect_err("an invalid loadout should be rejected");
    assert!(error.contains("ability `missing` does not belong to class `vanguard`"));
}

#[test]
fn profile_status_tracks_unsaved_changes_and_successful_saves() {
    let mut progression = PlayerProgression::default();
    let mut persistence = ProfilePersistence::loaded(&progression);

    assert_eq!(persistence.status_label(&progression), "SAVED");
    assert!(!persistence.has_unsaved_changes(&progression));
    assert!(!persistence.load_requires_confirmation(&progression));

    progression.award_experience(10);
    assert_eq!(persistence.status_label(&progression), "UNSAVED CHANGES");
    assert!(persistence.has_unsaved_changes(&progression));
    assert!(persistence.load_requires_confirmation(&progression));

    persistence.mark_saved(&progression);
    assert_eq!(persistence.status_label(&progression), "SAVED");
    assert!(!persistence.load_requires_confirmation(&progression));
}

#[test]
fn missing_and_broken_profile_states_are_explicit() {
    let progression = PlayerProgression::default();
    let mut persistence = ProfilePersistence::missing();

    assert_eq!(persistence.status_label(&progression), "NOT SAVED");
    assert!(persistence.has_unsaved_changes(&progression));
    assert!(!persistence.load_requires_confirmation(&progression));

    persistence.mark_error();
    assert_eq!(persistence.status_label(&progression), "FILE ERROR");

    persistence.mark_missing();
    assert_eq!(persistence.status_label(&progression), "NOT SAVED");
}
