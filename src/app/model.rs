use bevy::prelude::*;

use crate::{
    content::{AbilityDefinition, ContentCatalog},
    gameplay::{
        Ability, AbilitySpec, AttackWindup, Cooldown, Damage, DamageResult, EncounterPhase,
    },
    items::ItemInstance,
    progression::PlayerProgression,
    scripting::HeroAction,
    status_effects::StatusEffectSpec,
};

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum GameSet {
    Input,
    Cooldowns,
    Effects,
    Decisions,
    Actions,
    Damage,
    Resolution,
    Presentation,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum InputStage {
    Controls,
    ArenaReset,
    Progression,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ResolutionStage {
    CombatLog,
    Deaths,
    WaveAdvance,
}

#[derive(Component)]
pub(crate) struct Hero;

#[derive(Component)]
pub(crate) struct Enemy {
    pub(crate) attack_cooldown: Cooldown,
    pub(crate) attack_windup: AttackWindup,
    pub(crate) attack_range: f32,
    pub(crate) attack_cooldown_duration: f32,
    pub(crate) attack_windup_duration: f32,
    pub(crate) drop_table: String,
    pub(crate) xp_reward: u32,
    pub(crate) on_hit_effect: Option<StatusEffectSpec>,
}

#[derive(Component)]
pub(crate) struct HeroBrain {
    pub(crate) action: HeroAction,
    pub(crate) decision_cooldown: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct HeroAbility {
    pub(crate) definition: AbilityDefinition,
    runtime: Ability,
}

impl HeroAbility {
    fn new(definition: AbilityDefinition) -> Self {
        let runtime = Ability::new(AbilitySpec {
            base_damage: definition.base_damage,
            damage_scaling: definition.damage_scaling,
            range: definition.range,
            cooldown: definition.cooldown,
        });
        Self {
            definition,
            runtime,
        }
    }

    pub(crate) fn damage(&self, actor_stats: crate::stats::ActorStats) -> f32 {
        self.runtime.damage(actor_stats)
    }

    pub(crate) fn range(&self) -> f32 {
        self.runtime.range()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.runtime.is_ready()
    }

    pub(crate) fn remaining_cooldown(&self) -> f32 {
        self.runtime.remaining_cooldown()
    }

    pub(crate) fn activate(&mut self, attack_speed: f32) -> bool {
        self.runtime.activate(attack_speed)
    }

    fn tick(&mut self, delta_seconds: f32) {
        self.runtime.tick(delta_seconds);
    }
}

#[derive(Component, Clone, Debug)]
pub(crate) struct HeroCombat {
    pub(crate) class_id: String,
    pub(crate) class_name: String,
    abilities: Vec<HeroAbility>,
}

impl HeroCombat {
    pub(crate) fn from_profile(
        content: &ContentCatalog,
        progression: &PlayerProgression,
    ) -> Result<Self, String> {
        progression.validate_class_selection(content)?;
        let class = content
            .class(progression.class_id())
            .expect("validated profile class must exist");
        let abilities = progression
            .ability_loadout()
            .iter()
            .map(|ability_id| {
                content
                    .ability(ability_id)
                    .cloned()
                    .map(HeroAbility::new)
                    .ok_or_else(|| format!("unknown ability `{ability_id}`"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            class_id: class.id.clone(),
            class_name: class.name.clone(),
            abilities,
        })
    }

    pub(crate) fn ability(&self, id: &str) -> Option<&HeroAbility> {
        self.abilities
            .iter()
            .find(|ability| ability.definition.id == id)
    }

    pub(crate) fn ability_mut(&mut self, id: &str) -> Option<&mut HeroAbility> {
        self.abilities
            .iter_mut()
            .find(|ability| ability.definition.id == id)
    }

    pub(crate) fn abilities(&self) -> impl Iterator<Item = &HeroAbility> {
        self.abilities.iter()
    }

    pub(crate) fn tick(&mut self, delta_seconds: f32) {
        for ability in &mut self.abilities {
            ability.tick(delta_seconds);
        }
    }
}

#[derive(Component)]
pub(crate) struct Loot {
    pub(crate) item: ItemInstance,
    pub(super) base_y: f32,
    pub(super) phase: f32,
}

#[derive(Component)]
pub(super) struct ArenaEntity;

#[derive(Component)]
pub(super) struct HudText;

#[derive(Component)]
pub(super) struct WhirlwindEffect {
    pub(super) remaining: f32,
    pub(super) total: f32,
}

#[derive(Message, Clone, Debug)]
pub(super) struct DealDamage {
    pub(super) source: Entity,
    pub(super) target: Entity,
    pub(super) damage: Damage,
}

#[derive(Message, Clone, Debug)]
pub(super) struct DamageResolved {
    pub(super) source: Entity,
    pub(super) target: Entity,
    pub(super) result: DamageResult,
}

#[derive(Resource, Default)]
pub(crate) struct CombatLog {
    pub(super) last_hit: Option<DamageResolved>,
}

#[derive(Resource)]
pub(crate) struct ProgressionNotice(pub(crate) String);

#[derive(Resource)]
pub(crate) struct GameStats {
    pub(crate) kills: u32,
    pub(crate) loot_collected: u32,
    pub(crate) score: u32,
    pub(crate) interrupts: u32,
    pub(crate) phase: EncounterPhase,
    pub(crate) wave_index: usize,
    pub(super) reset_requested: bool,
}

impl Default for GameStats {
    fn default() -> Self {
        Self {
            kills: 0,
            loot_collected: 0,
            score: 0,
            interrupts: 0,
            phase: EncounterPhase::Combat,
            wave_index: 0,
            reset_requested: false,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub(crate) struct ArenaVisuals {
    pub(super) hero_mesh: Handle<Mesh>,
    pub(super) enemy_mesh: Handle<Mesh>,
    pub(super) loot_mesh: Handle<Mesh>,
    pub(super) ring_mesh: Handle<Mesh>,
    pub(super) hero_material: Handle<StandardMaterial>,
    pub(super) enemy_material: Handle<StandardMaterial>,
    pub(super) common_material: Handle<StandardMaterial>,
    pub(super) rare_material: Handle<StandardMaterial>,
    pub(super) epic_material: Handle<StandardMaterial>,
    pub(super) ring_material: Handle<StandardMaterial>,
}
