use std::fs;

use crate::{
    content::{AbilityTarget, CONTENT_PATH, ContentCatalog},
    items::{EquipmentSlot, LootRarity},
};

#[test]
fn shipped_content_catalog_is_valid_and_resolvable() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let encounter = content
        .encounter_at(0)
        .expect("first encounter should exist");
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
    assert_eq!(content.encounter_sequence.len(), 3);
    assert_eq!(
        content
            .encounter_for_wave(3)
            .expect("the encounter cycle should repeat")
            .id,
        "arena_one"
    );
    assert_eq!(spawn_count, 8);
    let vanguard = content
        .class(&content.default_class)
        .expect("default class should resolve");
    assert_eq!(vanguard.name, "Vanguard");
    assert_eq!(vanguard.abilities.len(), 5);
    assert_eq!(
        content
            .ability("guard")
            .expect("Guard should resolve")
            .target,
        AbilityTarget::SelfCast
    );
    assert_eq!(
        content
            .enemy_archetype("elite")
            .expect("elite should exist")
            .attack_windup,
        0.7
    );
    assert_eq!(
        content
            .enemy_archetype("elite")
            .and_then(|elite| elite.on_hit_effect.as_ref())
            .expect("elite should have an on-hit effect")
            .label(),
        "Poison"
    );
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
fn classes_cannot_reference_unknown_abilities() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen(
        "[\"strike\", \"cleave\", \"whirlwind\", \"charge\", \"guard\"]",
        "[\"strike\", \"missing\"]",
        1,
    );
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown class ability should fail validation");

    assert!(error.contains("class `vanguard` references unknown ability `missing`"));
}

#[test]
fn invalid_ability_combat_values_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("cooldown: 0.65", "cooldown: -1.0", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("a negative ability cooldown should fail validation");

    assert!(error.contains("ability `strike` has invalid combat values"));
}

#[test]
fn invalid_enemy_status_effects_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("duration: 3.0", "duration: -1.0", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an invalid on-hit status effect should fail validation");

    assert!(error.contains("enemy archetype `elite` has invalid on-hit effect"));
}

#[test]
fn invalid_enemy_attack_windups_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("attack_windup: 0.45", "attack_windup: 0.0", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("a zero-length attack windup should fail validation");

    assert!(error.contains("enemy archetype `grunt` contains invalid combat values"));
}

#[test]
fn enemy_stats_and_xp_scale_with_the_absolute_wave_number() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let grunt = content
        .enemy_archetype("grunt")
        .expect("grunt should exist");
    let first_wave = grunt.actor_stats_for_wave(content.wave_scaling, 0);
    let fourth_wave = grunt.actor_stats_for_wave(content.wave_scaling, 3);

    assert!(fourth_wave.max_health() > first_wave.max_health());
    assert!(fourth_wave.damage() > first_wave.damage());
    assert!(fourth_wave.defense() > first_wave.defense());
    assert!(fourth_wave.move_speed() > first_wave.move_speed());
    assert!(fourth_wave.attack_speed() > first_wave.attack_speed());
    assert!(
        content.wave_scaling.scale_xp(grunt.xp_reward, 3)
            > content.wave_scaling.scale_xp(grunt.xp_reward, 0)
    );
}

#[test]
fn invalid_wave_scaling_is_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("health_per_wave: 0.15", "health_per_wave: -0.1", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("negative wave scaling should fail validation");

    assert!(error.contains("wave scaling values must be finite and non-negative"));
}

#[test]
fn empty_encounter_sequences_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen(
        "encounter_sequence: [\"arena_one\", \"arena_two\", \"arena_three\"]",
        "encounter_sequence: []",
        1,
    );
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an empty encounter sequence should fail validation");

    assert!(error.contains("encounter sequence cannot be empty"));
}

#[test]
fn unknown_encounters_in_the_sequence_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("\"arena_two\"", "\"missing\"", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown sequenced encounter should fail validation");

    assert!(error.contains("encounter sequence references unknown encounter `missing`"));
}

#[test]
fn unknown_item_drops_are_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen(
        "(item: \"scout_boots\", weight: 100)",
        "(item: \"missing\", weight: 100)",
        1,
    );
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("an unknown item drop should fail validation");

    assert!(error.contains("invalid item entry `missing`"));
}

#[test]
fn generated_items_scale_and_roll_affixes_deterministically() {
    let content = ContentCatalog::load().expect("shipped content should load");
    let first = content
        .generate_drop("elite_drops", 8, 1234)
        .expect("drop should generate");
    let repeated = content
        .generate_drop("elite_drops", 8, 1234)
        .expect("drop should generate");

    assert_eq!(first, repeated);
    assert_eq!(first.item_level, 8);
    assert_eq!(first.affixes.len(), first.rarity.affix_count());
    assert!(first.power_score() > 0.0);

    let low_level = content
        .generate_drop("grunt_drops", 1, 99)
        .expect("drop should generate");
    let high_level = content
        .generate_drop("grunt_drops", 10, 99)
        .expect("drop should generate");
    assert!(high_level.power_score() > low_level.power_score());
}

#[test]
fn negative_item_modifiers_are_rejected_before_runtime() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTENT_PATH);
    let source = fs::read_to_string(path).expect("shipped content should be readable");
    let invalid = source.replacen("move_speed: (flat: 0.8)", "move_speed: (flat: -100.0)", 1);
    let error = ContentCatalog::from_ron(&invalid)
        .expect_err("a negative item bonus should fail validation");

    assert!(error.contains("item bonuses must be finite and non-negative"));
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
