use crate::progression::{PlayerProgression, UpgradeKind};

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
    let mut progression = PlayerProgression::default();
    progression.award_experience(80);
    progression.spend_point(UpgradeKind::Vitality);

    let encoded = progression.to_ron().expect("profile should serialize");
    let decoded = PlayerProgression::from_ron(&encoded).expect("profile should deserialize");

    assert_eq!(decoded, progression);
}

#[test]
fn inconsistent_progression_saves_are_rejected() {
    let encoded = PlayerProgression::default()
        .to_ron()
        .expect("default profile should serialize");
    let invalid = encoded.replacen("level: 1", "level: 3", 1);

    let error = PlayerProgression::from_ron(&invalid)
        .expect_err("unearned or missing points should invalidate a save");
    assert!(error.contains("upgrade points"));
}
