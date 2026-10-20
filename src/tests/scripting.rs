use rhai::{Dynamic, Map};

use crate::scripting::{
    CommandValidationError, HeroAction, HeroCommand, ScriptExecutionError, ScriptRuntime,
    ScriptWorldSnapshot,
};

fn decide(
    runtime: &mut ScriptRuntime,
    health_percent: f64,
    enemy_count: i64,
    nearest_distance: f64,
    whirlwind_ready: bool,
    loot_nearby: bool,
) -> HeroAction {
    runtime
        .decide(ScriptWorldSnapshot {
            health_percent,
            enemy_count,
            nearest_enemy_distance: nearest_distance,
            whirlwind_ready,
            loot_nearby,
        })
        .expect("the default hero script should return a valid command")
        .action
}

#[test]
fn default_script_selects_whirlwind_for_a_close_pack() {
    let mut runtime = ScriptRuntime::new();
    assert_eq!(
        decide(&mut runtime, 1.0, 4, 2.0, true, false),
        HeroAction::Whirlwind
    );
}

#[test]
fn default_script_attacks_a_single_enemy() {
    let mut runtime = ScriptRuntime::new();
    assert_eq!(
        decide(&mut runtime, 1.0, 1, 5.0, true, false),
        HeroAction::Attack
    );
}

#[test]
fn default_script_loots_after_combat() {
    let mut runtime = ScriptRuntime::new();
    assert_eq!(
        decide(&mut runtime, 1.0, 0, 999.0, true, true),
        HeroAction::Loot
    );
}

#[test]
fn low_health_takes_priority() {
    let mut runtime = ScriptRuntime::new();
    assert_eq!(
        decide(&mut runtime, 0.2, 4, 2.0, true, false),
        HeroAction::Retreat
    );
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
    assert!(
        error
            .to_string()
            .contains("attack, whirlwind, loot, retreat, or idle")
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
            whirlwind_ready: true,
            loot_nearby: false,
        })
        .expect_err("the bare string command should fail validation");

    assert!(matches!(error, ScriptExecutionError::InvalidCommand(_)));
    assert!(error.to_string().starts_with("INVALID COMMAND"));
}

#[test]
fn compile_failure_preserves_the_last_valid_script() {
    let mut runtime = ScriptRuntime::new();
    let expected = decide(&mut runtime, 1.0, 1, 2.0, true, false);

    assert!(runtime.install_source_for_test("fn decide(").is_err());
    assert_eq!(decide(&mut runtime, 1.0, 1, 2.0, true, false), expected);
}
