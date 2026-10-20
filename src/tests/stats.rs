use crate::stats::{ActorStats, BaseStats, StatModifier, StatModifiers};

fn base_stats() -> BaseStats {
    BaseStats {
        max_health: 100.0,
        damage: 10.0,
        defense: 5.0,
        move_speed: 4.0,
        attack_speed: 1.0,
    }
}

#[test]
fn flat_bonuses_are_applied_before_percentage_bonuses() {
    let stats = ActorStats::new(base_stats()).with_modifiers(StatModifiers {
        damage: StatModifier {
            flat: 5.0,
            increased: 0.2,
        },
        ..Default::default()
    });

    assert_eq!(stats.damage(), 18.0);
}

#[test]
fn each_actor_stat_resolves_its_own_modifier() {
    let stats = ActorStats::new(base_stats()).with_modifiers(StatModifiers {
        max_health: StatModifier {
            flat: 20.0,
            increased: 0.0,
        },
        defense: StatModifier {
            flat: 0.0,
            increased: 1.0,
        },
        move_speed: StatModifier {
            flat: 1.0,
            increased: 0.1,
        },
        attack_speed: StatModifier {
            flat: 0.0,
            increased: 0.25,
        },
        ..Default::default()
    });

    assert_eq!(stats.max_health(), 120.0);
    assert_eq!(stats.defense(), 10.0);
    assert_eq!(stats.move_speed(), 5.5);
    assert_eq!(stats.attack_speed(), 1.25);
}

#[test]
fn resolved_stats_cannot_be_negative() {
    let stats = ActorStats::new(base_stats()).with_modifiers(StatModifiers {
        damage: StatModifier {
            flat: -100.0,
            increased: 0.0,
        },
        attack_speed: StatModifier {
            flat: -100.0,
            increased: 0.0,
        },
        ..Default::default()
    });

    assert_eq!(stats.damage(), 0.0);
    assert_eq!(stats.attack_speed(), 0.05);
}

#[test]
fn modifier_sets_add_flat_and_percentage_values() {
    let first = StatModifiers {
        damage: StatModifier {
            flat: 2.0,
            increased: 0.1,
        },
        ..Default::default()
    };
    let second = StatModifiers {
        damage: StatModifier {
            flat: 3.0,
            increased: 0.2,
        },
        ..Default::default()
    };

    assert_eq!(
        first + second,
        StatModifiers {
            damage: StatModifier {
                flat: 5.0,
                increased: 0.3,
            },
            ..Default::default()
        }
    );
}
