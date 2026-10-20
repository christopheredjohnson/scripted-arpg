use std::{
    collections::VecDeque,
    fmt, fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::prelude::Resource;
use rhai::{AST, Array, Dynamic, Engine, ImmutableString, Map, Scope};

pub const HERO_SCRIPT: &str = "assets/hero.rhai";
pub const SCRIPT_API_VERSION: i64 = 3;
pub const SCRIPT_TRACE_CAPACITY: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeroAction {
    Cast(String),
    Loot,
    Retreat,
    Idle,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptAbilitySnapshot {
    pub id: String,
    pub name: String,
    pub target: String,
    pub ready: bool,
    pub cooldown: f64,
    pub range: f64,
    pub targets_in_range: i64,
    pub winding_up_targets_in_range: i64,
}

impl HeroAction {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "loot" => Some(Self::Loot),
            "retreat" => Some(Self::Retreat),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptWorldSnapshot {
    pub class_id: String,
    pub class_name: String,
    pub health_percent: f64,
    pub health: f64,
    pub max_health: f64,
    pub damage: f64,
    pub defense: f64,
    pub move_speed: f64,
    pub attack_speed: f64,
    pub level: i64,
    pub experience: i64,
    pub experience_to_next_level: i64,
    pub unspent_points: i64,
    pub inventory_count: i64,
    pub is_stunned: bool,
    pub active_effects: Vec<String>,
    pub phase: String,
    pub wave: i64,
    pub enemy_count: i64,
    pub enemies_winding_up: i64,
    pub nearest_enemy_distance: f64,
    pub nearest_enemy_health_percent: f64,
    pub nearest_enemy_damage: f64,
    pub nearest_enemy_defense: f64,
    pub nearest_enemy_move_speed: f64,
    pub nearest_enemy_is_winding_up: bool,
    pub nearest_enemy_windup_remaining: f64,
    pub loot_count: i64,
    pub loot_nearby: bool,
    pub abilities: Vec<ScriptAbilitySnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptTraceOutcome {
    Action(HeroAction),
    Error(String),
}

impl fmt::Display for ScriptTraceOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Action(action) => write!(formatter, "{action:?}"),
            Self::Error(error) => formatter.write_str(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptTraceEntry {
    pub sequence: u64,
    pub reload: u32,
    pub snapshot: ScriptWorldSnapshot,
    pub outcome: ScriptTraceOutcome,
    pub elapsed: Duration,
}

impl ScriptTraceEntry {
    pub fn summary(&self) -> String {
        format!(
            "#{:04}/r{} · HP {:>3.0}% · E {} · wind {} · near {:.1} · {}/{} ready · loot {} · {} · {}µs",
            self.sequence,
            self.reload,
            self.snapshot.health_percent * 100.0,
            self.snapshot.enemy_count,
            self.snapshot.enemies_winding_up,
            self.snapshot.nearest_enemy_distance,
            self.snapshot
                .abilities
                .iter()
                .filter(|ability| ability.ready)
                .count(),
            self.snapshot.abilities.len(),
            if self.snapshot.loot_nearby {
                "yes"
            } else {
                "no"
            },
            self.outcome,
            self.elapsed.as_micros(),
        )
    }
}

impl ScriptWorldSnapshot {
    fn to_rhai_map(&self) -> Map {
        let mut hero = Map::new();
        hero.insert("health_percent".into(), self.health_percent.into());
        hero.insert("health".into(), self.health.into());
        hero.insert("max_health".into(), self.max_health.into());
        hero.insert("damage".into(), self.damage.into());
        hero.insert("defense".into(), self.defense.into());
        hero.insert("move_speed".into(), self.move_speed.into());
        hero.insert("attack_speed".into(), self.attack_speed.into());
        hero.insert("level".into(), self.level.into());
        hero.insert("experience".into(), self.experience.into());
        hero.insert(
            "experience_to_next_level".into(),
            self.experience_to_next_level.into(),
        );
        hero.insert("unspent_points".into(), self.unspent_points.into());
        hero.insert("inventory_count".into(), self.inventory_count.into());
        hero.insert("class_id".into(), self.class_id.clone().into());
        hero.insert("class_name".into(), self.class_name.clone().into());
        hero.insert("is_stunned".into(), self.is_stunned.into());
        let effects: Array = self
            .active_effects
            .iter()
            .cloned()
            .map(Dynamic::from)
            .collect();
        hero.insert("effects".into(), effects.into());

        let mut encounter = Map::new();
        encounter.insert("phase".into(), self.phase.clone().into());
        encounter.insert("wave".into(), self.wave.into());
        encounter.insert("enemy_count".into(), self.enemy_count.into());
        encounter.insert("enemies_winding_up".into(), self.enemies_winding_up.into());
        encounter.insert(
            "nearest_enemy_distance".into(),
            self.nearest_enemy_distance.into(),
        );
        encounter.insert(
            "nearest_enemy_health_percent".into(),
            self.nearest_enemy_health_percent.into(),
        );
        encounter.insert(
            "nearest_enemy_damage".into(),
            self.nearest_enemy_damage.into(),
        );
        encounter.insert(
            "nearest_enemy_defense".into(),
            self.nearest_enemy_defense.into(),
        );
        encounter.insert(
            "nearest_enemy_move_speed".into(),
            self.nearest_enemy_move_speed.into(),
        );
        encounter.insert(
            "nearest_enemy_is_winding_up".into(),
            self.nearest_enemy_is_winding_up.into(),
        );
        encounter.insert(
            "nearest_enemy_windup_remaining".into(),
            self.nearest_enemy_windup_remaining.into(),
        );
        encounter.insert("loot_count".into(), self.loot_count.into());
        encounter.insert("loot_nearby".into(), self.loot_nearby.into());

        let mut abilities = Map::new();
        for ability in &self.abilities {
            let mut snapshot = Map::new();
            snapshot.insert("name".into(), ability.name.clone().into());
            snapshot.insert("target".into(), ability.target.clone().into());
            snapshot.insert("ready".into(), ability.ready.into());
            snapshot.insert("cooldown".into(), ability.cooldown.into());
            snapshot.insert("range".into(), ability.range.into());
            snapshot.insert("targets_in_range".into(), ability.targets_in_range.into());
            snapshot.insert(
                "winding_up_targets_in_range".into(),
                ability.winding_up_targets_in_range.into(),
            );
            abilities.insert(ability.id.clone().into(), snapshot.into());
        }

        let mut world = Map::new();
        world.insert("api_version".into(), SCRIPT_API_VERSION.into());
        world.insert("hero".into(), hero.into());
        world.insert("encounter".into(), encounter.into());
        world.insert("abilities".into(), abilities.into());
        world
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeroCommand {
    pub action: HeroAction,
}

impl HeroCommand {
    pub(crate) fn from_dynamic(value: Dynamic) -> Result<Self, CommandValidationError> {
        let received_type = value.type_name().to_string();
        let Some(mut command) = value.try_cast::<Map>() else {
            return Err(CommandValidationError::ExpectedMap(received_type));
        };
        let Some(action) = command.remove("action") else {
            return Err(CommandValidationError::MissingAction);
        };
        let received_type = action.type_name().to_string();
        let Some(action) = action.try_cast::<ImmutableString>() else {
            return Err(CommandValidationError::ActionMustBeString(received_type));
        };
        let action = if action.as_str().trim().eq_ignore_ascii_case("cast") {
            let Some(ability) = command.remove("ability") else {
                return Err(CommandValidationError::MissingAbility);
            };
            let received_type = ability.type_name().to_string();
            let Some(ability) = ability.try_cast::<ImmutableString>() else {
                return Err(CommandValidationError::AbilityMustBeString(received_type));
            };
            let ability = ability.trim();
            if ability.is_empty() {
                return Err(CommandValidationError::EmptyAbility);
            }
            HeroAction::Cast(ability.to_ascii_lowercase())
        } else {
            let Some(action) = HeroAction::parse(action.as_str()) else {
                return Err(CommandValidationError::UnknownAction(action.to_string()));
            };
            action
        };
        Ok(Self { action })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandValidationError {
    ExpectedMap(String),
    MissingAction,
    ActionMustBeString(String),
    MissingAbility,
    AbilityMustBeString(String),
    EmptyAbility,
    UnavailableAbility(String),
    UnknownAction(String),
}

impl fmt::Display for CommandValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedMap(received) => write!(
                formatter,
                "decide(world) must return an object like #{{ action: \"attack\" }}, got {received}"
            ),
            Self::MissingAction => write!(formatter, "command is missing the `action` field"),
            Self::ActionMustBeString(received) => {
                write!(
                    formatter,
                    "command `action` must be a string, got {received}"
                )
            }
            Self::MissingAbility => {
                write!(formatter, "cast command is missing the `ability` field")
            }
            Self::AbilityMustBeString(received) => {
                write!(
                    formatter,
                    "command `ability` must be a string, got {received}"
                )
            }
            Self::EmptyAbility => write!(formatter, "command `ability` must not be empty"),
            Self::UnavailableAbility(ability) => {
                write!(formatter, "ability `{ability}` is not equipped")
            }
            Self::UnknownAction(action) => write!(
                formatter,
                "unknown action `{action}`; expected cast, loot, retreat, or idle"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptExecutionError {
    Runtime(String),
    InvalidCommand(CommandValidationError),
}

impl fmt::Display for ScriptExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "RUNTIME ERROR · {error}"),
            Self::InvalidCommand(error) => write!(formatter, "INVALID COMMAND · {error}"),
        }
    }
}

#[derive(Resource)]
pub struct ScriptRuntime {
    engine: Engine,
    ast: AST,
    status: String,
    reload_count: u32,
    observed_disk_source: Option<String>,
    decision_count: u64,
    trace: VecDeque<ScriptTraceEntry>,
}

impl ScriptRuntime {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_operations(2_000);
        engine.set_max_call_levels(8);
        engine.set_max_expr_depths(32, 16);
        engine.set_max_variables(32);
        engine.disable_symbol("import");
        engine.disable_symbol("eval");

        let fallback = r#"
            fn decide(world) {
                #{ action: "idle" }
            }
        "#;
        let ast = engine
            .compile(fallback)
            .expect("the built-in fallback hero script must compile");

        let mut runtime = Self {
            engine,
            ast,
            status: "Using fallback script".to_string(),
            reload_count: 0,
            observed_disk_source: None,
            decision_count: 0,
            trace: VecDeque::with_capacity(SCRIPT_TRACE_CAPACITY),
        };
        runtime.reload();
        runtime
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn latest_trace(&self) -> Option<&ScriptTraceEntry> {
        self.trace.back()
    }

    pub fn trace_history(&self) -> impl DoubleEndedIterator<Item = &ScriptTraceEntry> {
        self.trace.iter()
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    pub fn decide(
        &mut self,
        snapshot: ScriptWorldSnapshot,
    ) -> Result<HeroCommand, ScriptExecutionError> {
        let started = Instant::now();
        let mut scope = Scope::new();
        let result = self
            .engine
            .call_fn::<Dynamic>(&mut scope, &self.ast, "decide", (snapshot.to_rhai_map(),))
            .map_err(|error| ScriptExecutionError::Runtime(error.to_string()))
            .and_then(|value| {
                let command = HeroCommand::from_dynamic(value)
                    .map_err(ScriptExecutionError::InvalidCommand)?;
                if let HeroAction::Cast(ability_id) = &command.action
                    && !snapshot
                        .abilities
                        .iter()
                        .any(|ability| ability.id == *ability_id)
                {
                    return Err(ScriptExecutionError::InvalidCommand(
                        CommandValidationError::UnavailableAbility(ability_id.clone()),
                    ));
                }
                Ok(command)
            });
        match &result {
            Ok(_)
                if self.status.starts_with("RUNTIME ERROR")
                    || self.status.starts_with("INVALID COMMAND") =>
            {
                self.status = format!(
                    "RUNNING · API v{SCRIPT_API_VERSION} · reload #{}",
                    self.reload_count
                );
            }
            Err(error) => self.status = error.to_string(),
            Ok(_) => {}
        }
        self.record_trace(snapshot, &result, started.elapsed());
        result
    }

    fn record_trace(
        &mut self,
        snapshot: ScriptWorldSnapshot,
        result: &Result<HeroCommand, ScriptExecutionError>,
        elapsed: Duration,
    ) {
        self.decision_count = self.decision_count.saturating_add(1);
        if self.trace.len() == SCRIPT_TRACE_CAPACITY {
            self.trace.pop_front();
        }
        let outcome = match result {
            Ok(command) => ScriptTraceOutcome::Action(command.action.clone()),
            Err(error) => ScriptTraceOutcome::Error(error.to_string()),
        };
        self.trace.push_back(ScriptTraceEntry {
            sequence: self.decision_count,
            reload: self.reload_count,
            snapshot,
            outcome,
            elapsed,
        });
    }

    fn install_source(&mut self, source: String) -> Result<(), String> {
        let ast = self
            .engine
            .compile(&source)
            .map_err(|error| error.to_string())?;
        self.ast = ast;
        self.reload_count += 1;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn install_source_for_test(&mut self, source: &str) -> Result<(), String> {
        self.install_source(source.to_string())
    }

    fn script_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(HERO_SCRIPT)
    }

    pub fn reload(&mut self) {
        let path = Self::script_path();
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                self.status = format!("Could not read {}: {error}", path.display());
                return;
            }
        };

        self.observed_disk_source = Some(source.clone());
        match self.install_source(source) {
            Ok(()) => {
                self.status = format!(
                    "RUNNING · API v{SCRIPT_API_VERSION} · reload #{}",
                    self.reload_count
                );
            }
            Err(error) => {
                self.status = format!("COMPILE ERROR · {error}");
            }
        }
    }

    pub fn reload_if_changed(&mut self) -> bool {
        let path = Self::script_path();
        let Ok(source) = fs::read_to_string(&path) else {
            return false;
        };
        self.reload_changed_source(source)
    }

    fn reload_changed_source(&mut self, source: String) -> bool {
        if self.observed_disk_source.as_deref() == Some(source.as_str()) {
            return false;
        }
        self.observed_disk_source = Some(source.clone());

        match self.install_source(source) {
            Ok(()) => {
                self.status = format!(
                    "RUNNING · API v{SCRIPT_API_VERSION} · reload #{} · external save detected",
                    self.reload_count
                );
                true
            }
            Err(error) => {
                self.status = format!("COMPILE ERROR · {error}");
                false
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn reload_changed_source_for_test(&mut self, source: &str) -> bool {
        self.reload_changed_source(source.to_string())
    }
}
