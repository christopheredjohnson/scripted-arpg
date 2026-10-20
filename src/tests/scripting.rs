use rhai::{Dynamic, Map};

use crate::scripting::{
    CommandValidationError, HeroAction, HeroCommand, SCRIPT_TRACE_CAPACITY, ScriptAbilitySnapshot,
    ScriptExecutionError, ScriptRuntime, ScriptTraceOutcome, ScriptWorldSnapshot,
};

fn decide(
    runtime: &mut ScriptRuntime,
    health_percent: f64,
    enemy_count: i64,
    nearest_distance: f64,
    whirlwind_ready: bool,
    loot_nearby: bool,
) -> HeroAction {
    let mut whirlwind = ability("whirlwind", whirlwind_ready, 3.5);
    whirlwind.targets_in_range = if nearest_distance <= 3.5 {
        enemy_count
    } else {
        0
    };
    runtime
        .decide(ScriptWorldSnapshot {
            health_percent,
            enemy_count,
            nearest_enemy_distance: nearest_distance,
            loot_count: if loot_nearby { 1 } else { 0 },
            loot_nearby,
            abilities: vec![
                ability("strike", true, 1.45),
                ability("cleave", true, 2.0),
                whirlwind,
                ability("charge", true, 1.25),
                ability("guard", true, 0.0),
            ],
            ..Default::default()
        })
        .expect("the default hero script should return a valid command")
        .action
}

fn ability(id: &str, ready: bool, range: f64) -> ScriptAbilitySnapshot {
    ScriptAbilitySnapshot {
        id: id.to_string(),
        name: id.to_string(),
        target: "test".to_string(),
        ready,
        cooldown: if ready { 0.0 } else { 1.5 },
        range,
        targets_in_range: 0,
        winding_up_targets_in_range: 0,
    }
}

fn default_test_runtime() -> ScriptRuntime {
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test(include_str!("../../assets/hero.rhai"))
        .expect("the test hero script should compile");
    runtime
}

#[test]
fn default_script_selects_whirlwind_for_a_close_pack() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 1.0, 4, 2.0, true, false),
        HeroAction::Cast("whirlwind".to_string())
    );
}

#[test]
fn default_script_charges_a_distant_single_enemy() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 1.0, 1, 5.0, true, false),
        HeroAction::Cast("charge".to_string())
    );
}

#[test]
fn default_script_loots_after_combat() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 1.0, 0, 999.0, true, true),
        HeroAction::Loot
    );
}

#[test]
fn default_script_collects_loot_that_is_not_nearby_yet() {
    let mut runtime = default_test_runtime();
    let command = runtime
        .decide(ScriptWorldSnapshot {
            health_percent: 0.2,
            nearest_enemy_distance: 999.0,
            loot_count: 1,
            abilities: vec![
                ability("strike", true, 1.45),
                ability("cleave", true, 2.0),
                ability("whirlwind", true, 3.5),
                ability("charge", true, 1.25),
                ability("guard", true, 0.0),
            ],
            ..Default::default()
        })
        .expect("the starter should return a valid loot command");

    assert_eq!(command.action, HeroAction::Loot);
}

#[test]
fn default_script_reports_idle_while_stunned() {
    let mut runtime = default_test_runtime();
    let command = runtime
        .decide(ScriptWorldSnapshot {
            health_percent: 1.0,
            is_stunned: true,
            enemy_count: 4,
            nearest_enemy_distance: 2.0,
            ..Default::default()
        })
        .expect("the starter should return a valid idle command");

    assert_eq!(command.action, HeroAction::Idle);
}

#[test]
fn low_health_takes_priority() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 0.2, 1, 2.0, true, false),
        HeroAction::Cast("guard".to_string())
    );
}

#[test]
fn default_script_interrupts_a_windup_before_retreating() {
    let mut runtime = default_test_runtime();
    let mut whirlwind = ability("whirlwind", true, 3.5);
    whirlwind.targets_in_range = 1;
    whirlwind.winding_up_targets_in_range = 1;
    let command = runtime
        .decide(ScriptWorldSnapshot {
            health_percent: 0.2,
            enemy_count: 1,
            enemies_winding_up: 1,
            nearest_enemy_distance: 1.0,
            nearest_enemy_is_winding_up: true,
            nearest_enemy_windup_remaining: 0.2,
            abilities: vec![
                ability("strike", true, 1.45),
                ability("cleave", true, 2.0),
                whirlwind,
                ability("charge", true, 1.25),
                ability("guard", true, 0.0),
            ],
            ..Default::default()
        })
        .expect("the starter should return a valid interrupt command");

    assert_eq!(command.action, HeroAction::Cast("whirlwind".to_string()));
}

#[test]
fn surrounded_low_health_hero_uses_whirlwind_instead_of_looping_retreat() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 0.2, 4, 2.0, true, false),
        HeroAction::Cast("whirlwind".to_string())
    );
}

#[test]
fn surrounded_low_health_hero_keeps_attacking_while_whirlwind_recharges() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 0.2, 4, 2.0, false, false),
        HeroAction::Cast("cleave".to_string())
    );
}

#[test]
fn api_v3_exposes_class_abilities_combat_progression_and_effect_state() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test(
            r#"
                fn decide(world) {
                    if world.api_version != 3 { return #{ action: "idle" }; }
                    if world.hero.class_id != "vanguard" { return #{ action: "idle" }; }
                    if world.hero.class_name != "Vanguard" { return #{ action: "idle" }; }
                    if world.hero.health != 75.0 { return #{ action: "idle" }; }
                    if world.hero.max_health != 100.0 { return #{ action: "idle" }; }
                    if world.hero.damage != 14.0 { return #{ action: "idle" }; }
                    if world.hero.defense != 6.0 { return #{ action: "idle" }; }
                    if world.hero.move_speed != 4.5 { return #{ action: "idle" }; }
                    if world.hero.attack_speed != 1.25 { return #{ action: "idle" }; }
                    if world.hero.level != 3 { return #{ action: "idle" }; }
                    if world.hero.experience != 20 { return #{ action: "idle" }; }
                    if world.hero.experience_to_next_level != 80 {
                        return #{ action: "idle" };
                    }
                    if world.hero.unspent_points != 2 { return #{ action: "idle" }; }
                    if world.hero.inventory_count != 7 { return #{ action: "idle" }; }
                    if !world.hero.is_stunned { return #{ action: "idle" }; }
                    if world.hero.effects[0] != "Poison" { return #{ action: "idle" }; }
                    if world.hero.effects[1] != "Stun" { return #{ action: "idle" }; }
                    if world.encounter.phase != "combat" { return #{ action: "idle" }; }
                    if world.encounter.wave != 5 { return #{ action: "idle" }; }
                    if world.encounter.enemy_count != 4 { return #{ action: "idle" }; }
                    if world.encounter.enemies_winding_up != 2 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_distance != 3.0 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_health_percent != 0.4 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_damage != 11.0 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_defense != 3.0 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_move_speed != 2.5 {
                        return #{ action: "idle" };
                    }
                    if !world.encounter.nearest_enemy_is_winding_up {
                        return #{ action: "idle" };
                    }
                    if world.encounter.nearest_enemy_windup_remaining != 0.25 {
                        return #{ action: "idle" };
                    }
                    if world.encounter.loot_count != 2 { return #{ action: "idle" }; }
                    if !world.encounter.loot_nearby { return #{ action: "idle" }; }
                    if world.abilities.whirlwind.name != "Whirlwind" {
                        return #{ action: "idle" };
                    }
                    if world.abilities.whirlwind.target != "area" {
                        return #{ action: "idle" };
                    }
                    if world.abilities.whirlwind.ready { return #{ action: "idle" }; }
                    if world.abilities.whirlwind.cooldown != 1.5 {
                        return #{ action: "idle" };
                    }
                    if world.abilities.whirlwind.range != 3.5 {
                        return #{ action: "idle" };
                    }
                    if world.abilities.whirlwind.targets_in_range != 3 {
                        return #{ action: "idle" };
                    }
                    if world.abilities.whirlwind.winding_up_targets_in_range != 1 {
                        return #{ action: "idle" };
                    }
                    #{ action: "retreat" }
                }
            "#,
        )
        .expect("the API v3 test script should compile");

    let command = runtime
        .decide(ScriptWorldSnapshot {
            class_id: "vanguard".to_string(),
            class_name: "Vanguard".to_string(),
            health_percent: 0.75,
            health: 75.0,
            max_health: 100.0,
            damage: 14.0,
            defense: 6.0,
            move_speed: 4.5,
            attack_speed: 1.25,
            level: 3,
            experience: 20,
            experience_to_next_level: 80,
            unspent_points: 2,
            inventory_count: 7,
            is_stunned: true,
            active_effects: vec!["Poison".to_string(), "Stun".to_string()],
            phase: "combat".to_string(),
            wave: 5,
            enemy_count: 4,
            enemies_winding_up: 2,
            nearest_enemy_distance: 3.0,
            nearest_enemy_health_percent: 0.4,
            nearest_enemy_damage: 11.0,
            nearest_enemy_defense: 3.0,
            nearest_enemy_move_speed: 2.5,
            nearest_enemy_is_winding_up: true,
            nearest_enemy_windup_remaining: 0.25,
            loot_count: 2,
            loot_nearby: true,
            abilities: vec![ScriptAbilitySnapshot {
                id: "whirlwind".to_string(),
                name: "Whirlwind".to_string(),
                target: "area".to_string(),
                ready: false,
                cooldown: 1.5,
                range: 3.5,
                targets_in_range: 3,
                winding_up_targets_in_range: 1,
            }],
        })
        .expect("API v3 fields should be accessible");

    assert_eq!(command.action, HeroAction::Retreat);
}

#[test]
fn command_must_be_an_object() {
    let error = HeroCommand::from_dynamic(Dynamic::from("attack"))
        .expect_err("a bare action string should be rejected");

    assert!(matches!(error, CommandValidationError::ExpectedMap(_)));
    assert!(error.to_string().contains("must return an object"));
}

#[test]
fn command_requires_an_action_field() {
    let error = HeroCommand::from_dynamic(Dynamic::from(Map::new()))
        .expect_err("a command without an action should be rejected");

    assert_eq!(error, CommandValidationError::MissingAction);
    assert!(error.to_string().contains("missing the `action` field"));
}

#[test]
fn command_rejects_unknown_actions_with_the_allowed_values() {
    let mut command = Map::new();
    command.insert("action".into(), "teleport".into());
    let error = HeroCommand::from_dynamic(Dynamic::from(command))
        .expect_err("an unknown action should be rejected");

    assert_eq!(
        error,
        CommandValidationError::UnknownAction("teleport".to_string())
    );
    assert!(error.to_string().contains("cast, loot, retreat, or idle"));
}

#[test]
fn removed_fixed_combat_actions_are_rejected() {
    for action in ["attack", "whirlwind"] {
        let mut command = Map::new();
        command.insert("action".into(), action.into());
        assert_eq!(
            HeroCommand::from_dynamic(Dynamic::from(command))
                .expect_err("fixed combat actions should no longer be accepted"),
            CommandValidationError::UnknownAction(action.to_string())
        );
    }
}

#[test]
fn cast_command_requires_a_nonempty_ability_id() {
    let mut missing = Map::new();
    missing.insert("action".into(), "cast".into());
    assert_eq!(
        HeroCommand::from_dynamic(Dynamic::from(missing))
            .expect_err("cast without an ability should fail"),
        CommandValidationError::MissingAbility
    );

    let mut empty = Map::new();
    empty.insert("action".into(), "cast".into());
    empty.insert("ability".into(), "   ".into());
    assert_eq!(
        HeroCommand::from_dynamic(Dynamic::from(empty))
            .expect_err("cast with an empty ability should fail"),
        CommandValidationError::EmptyAbility
    );
}

#[test]
fn cast_command_normalizes_the_ability_id() {
    let mut command = Map::new();
    command.insert("action".into(), "CAST".into());
    command.insert("ability".into(), "WhirlWind".into());

    assert_eq!(
        HeroCommand::from_dynamic(Dynamic::from(command))
            .expect("valid cast should parse")
            .action,
        HeroAction::Cast("whirlwind".to_string())
    );
}

#[test]
fn runtime_rejects_casting_an_ability_that_is_not_equipped() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test("fn decide(world) { #{ action: \"cast\", ability: \"meteor\" } }")
        .expect("unknown ability test script should compile");

    let error = runtime
        .decide(ScriptWorldSnapshot {
            abilities: vec![ability("strike", true, 1.45)],
            ..Default::default()
        })
        .expect_err("an unequipped ability should fail command validation");
    assert_eq!(
        error,
        ScriptExecutionError::InvalidCommand(CommandValidationError::UnavailableAbility(
            "meteor".to_string()
        ))
    );
}

#[test]
fn runtime_reports_invalid_command_results() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .install_source_for_test("fn decide(world) { \"attack\" }")
        .expect("the intentionally invalid command script should compile");

    let error = runtime
        .decide(ScriptWorldSnapshot {
            health_percent: 1.0,
            enemy_count: 1,
            nearest_enemy_distance: 2.0,
            loot_nearby: false,
            ..Default::default()
        })
        .expect_err("the bare string command should fail validation");

    assert!(matches!(error, ScriptExecutionError::InvalidCommand(_)));
    assert!(error.to_string().starts_with("INVALID COMMAND"));
    let trace = runtime
        .latest_trace()
        .expect("the failed decision should be traced");
    assert_eq!(trace.sequence, 1);
    assert_eq!(trace.snapshot.enemy_count, 1);
    assert!(matches!(trace.outcome, ScriptTraceOutcome::Error(_)));
}

#[test]
fn compile_failure_preserves_the_last_valid_script() {
    let mut runtime = ScriptRuntime::new();
    let expected = decide(&mut runtime, 1.0, 1, 2.0, true, false);

    assert!(runtime.install_source_for_test("fn decide(").is_err());
    assert_eq!(decide(&mut runtime, 1.0, 1, 2.0, true, false), expected);
}

#[test]
fn external_saves_auto_reload_once_and_preserve_the_last_valid_script() {
    let mut runtime = ScriptRuntime::new();
    let changed = r#"fn decide(world) { #{ action: "retreat" } }"#;

    assert!(runtime.reload_changed_source_for_test(changed));
    assert_eq!(
        decide(&mut runtime, 1.0, 1, 2.0, true, false),
        HeroAction::Retreat
    );
    assert!(!runtime.reload_changed_source_for_test(changed));

    assert!(!runtime.reload_changed_source_for_test("fn decide("));
    assert_eq!(
        decide(&mut runtime, 1.0, 1, 2.0, true, false),
        HeroAction::Retreat
    );
    assert!(runtime.status().starts_with("COMPILE ERROR"));
}

#[test]
fn successful_decisions_capture_inputs_outputs_and_reload() {
    let mut runtime = default_test_runtime();
    assert_eq!(
        decide(&mut runtime, 0.8, 1, 4.0, true, false),
        HeroAction::Cast("charge".to_string())
    );

    let trace = runtime.latest_trace().expect("decision should be traced");
    assert_eq!(trace.sequence, 1);
    assert_eq!(trace.snapshot.health_percent, 0.8);
    assert_eq!(trace.snapshot.nearest_enemy_distance, 4.0);
    assert_eq!(
        trace.outcome,
        ScriptTraceOutcome::Action(HeroAction::Cast("charge".to_string()))
    );
    assert!(trace.reload > 0);
    assert!(trace.summary().contains("charge"));
}

#[test]
fn decision_trace_is_bounded_and_can_be_cleared() {
    let mut runtime = default_test_runtime();
    for _ in 0..SCRIPT_TRACE_CAPACITY + 5 {
        decide(&mut runtime, 1.0, 1, 3.0, false, false);
    }

    assert_eq!(runtime.trace_history().count(), SCRIPT_TRACE_CAPACITY);
    assert_eq!(
        runtime
            .trace_history()
            .next()
            .expect("bounded trace should contain decisions")
            .sequence,
        6
    );
    runtime.clear_trace();
    assert!(runtime.latest_trace().is_none());
}
