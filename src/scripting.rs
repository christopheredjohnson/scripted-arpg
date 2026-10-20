use std::{fmt, fs, path::PathBuf};

use bevy::prelude::Resource;
use rhai::{AST, Dynamic, Engine, ImmutableString, Map, Scope};

pub const HERO_SCRIPT: &str = "assets/hero.rhai";
pub const SCRIPT_API_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroAction {
    Attack,
    Whirlwind,
    Loot,
    Retreat,
    Idle,
}

impl HeroAction {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "attack" => Some(Self::Attack),
            "whirlwind" => Some(Self::Whirlwind),
            "loot" => Some(Self::Loot),
            "retreat" => Some(Self::Retreat),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptWorldSnapshot {
    pub health_percent: f64,
    pub enemy_count: i64,
    pub nearest_enemy_distance: f64,
    pub whirlwind_ready: bool,
    pub loot_nearby: bool,
}

impl ScriptWorldSnapshot {
    fn into_rhai_map(self) -> Map {
        let mut hero = Map::new();
        hero.insert("health_percent".into(), self.health_percent.into());

        let mut encounter = Map::new();
        encounter.insert("enemy_count".into(), self.enemy_count.into());
        encounter.insert(
            "nearest_enemy_distance".into(),
            self.nearest_enemy_distance.into(),
        );
        encounter.insert("loot_nearby".into(), self.loot_nearby.into());

        let mut abilities = Map::new();
        abilities.insert("whirlwind_ready".into(), self.whirlwind_ready.into());

        let mut world = Map::new();
        world.insert("api_version".into(), SCRIPT_API_VERSION.into());
        world.insert("hero".into(), hero.into());
        world.insert("encounter".into(), encounter.into());
        world.insert("abilities".into(), abilities.into());
        world
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        let Some(action) = HeroAction::parse(action.as_str()) else {
            return Err(CommandValidationError::UnknownAction(action.to_string()));
        };
        Ok(Self { action })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandValidationError {
    ExpectedMap(String),
    MissingAction,
    ActionMustBeString(String),
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
            Self::UnknownAction(action) => write!(
                formatter,
                "unknown action `{action}`; expected attack, whirlwind, loot, retreat, or idle"
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
    source: String,
    status: String,
    reload_count: u32,
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
            source: fallback.trim().to_string(),
            status: "Using fallback script".to_string(),
            reload_count: 0,
        };
        runtime.reload();
        runtime
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn decide(
        &mut self,
        snapshot: ScriptWorldSnapshot,
    ) -> Result<HeroCommand, ScriptExecutionError> {
        let mut scope = Scope::new();
        let value = match self.engine.call_fn::<Dynamic>(
            &mut scope,
            &self.ast,
            "decide",
            (snapshot.into_rhai_map(),),
        ) {
            Ok(value) => value,
            Err(error) => {
                let error = ScriptExecutionError::Runtime(error.to_string());
                self.status = error.to_string();
                return Err(error);
            }
        };

        let result = HeroCommand::from_dynamic(value).map_err(ScriptExecutionError::InvalidCommand);
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
        result
    }

    fn install_source(&mut self, source: String) -> Result<(), String> {
        let ast = self
            .engine
            .compile(&source)
            .map_err(|error| error.to_string())?;
        self.ast = ast;
        self.source = source;
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
}
