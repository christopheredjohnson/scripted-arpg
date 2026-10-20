# Scripted ARPG MVP

A code-first 3D combat sandbox built with Rust, Bevy, and Rhai. The hero is controlled by the program in `assets/hero.rhai` rather than direct movement input.

## Run

```bash
cargo run
```

The first build is large because Bevy and its rendering stack must compile.

## Controls

- `R`: force-reload `assets/hero.rhai` (saved changes also reload automatically)
- `T`: clear the script decision trace
- `Space`: restart the arena
- `1`: spend an upgrade point on Power
- `2`: spend an upgrade point on Vitality
- `3`: spend an upgrade point on Swiftness
- `S`: save progression to `saves/profile.ron`
- `L`: load saved progression (press twice to discard unsaved changes)
- `Escape`: quit

## Hero program

The game calls this API v3 function five times per second. It receives a world
snapshot and returns a validated command object:

```rhai
fn decide(world) {
    if world.hero.is_stunned {
        return #{ action: "idle" };
    }
    if world.abilities.whirlwind.winding_up_targets_in_range > 0
        && world.abilities.whirlwind.ready
    {
        return #{ action: "cast", ability: "whirlwind" };
    }
    if world.hero.health_percent <= 0.35 && world.abilities.guard.ready {
        return #{ action: "cast", ability: "guard" };
    }
    if world.encounter.enemy_count >= 2 && world.abilities.cleave.ready {
        return #{ action: "cast", ability: "cleave" };
    }
    if world.encounter.loot_count > 0 {
        return #{ action: "loot" };
    }
    #{ action: "cast", ability: "strike" }
}
```

The shipped starter uses all five Vanguard abilities, interrupts telegraphed
attacks, avoids retreat loops when surrounded, and collects every remaining
drop. Edit `assets/hero.rhai` to change those priorities.

The v3 snapshot exposes these fields:

| Group | Fields |
| --- | --- |
| World | `api_version` |
| Hero combat | `hero.class_id`, `class_name`, `health_percent`, `health`, `max_health`, `damage`, `defense`, `move_speed`, `attack_speed`, `is_stunned`, `effects` |
| Hero progression | `hero.level`, `experience`, `experience_to_next_level`, `unspent_points`, `inventory_count` |
| Encounter | `encounter.phase`, `wave`, `enemy_count`, `enemies_winding_up`, `loot_count`, `loot_nearby` |
| Nearest enemy | `encounter.nearest_enemy_distance`, `nearest_enemy_health_percent`, `nearest_enemy_damage`, `nearest_enemy_defense`, `nearest_enemy_move_speed`, `nearest_enemy_is_winding_up`, `nearest_enemy_windup_remaining` |
| Abilities | `abilities.<id>.name`, `target`, `ready`, `cooldown`, `range`, `targets_in_range`, `winding_up_targets_in_range` |

`encounter.phase` is `"combat"`, `"looting"`, `"victory"`, or `"defeat"`;
`encounter.wave` is one-based; cooldowns and windups are remaining seconds; and
`hero.effects` is an array of active effect labels. Each equipped ability gets
its own map, such as `world.abilities.guard.ready`. The per-ability target
counts let scripts reason about any future class without adding fixed snapshot
fields. When no enemy exists, its distance is `999.0` and its other fields are
`0.0`.

Use `#{ action: "cast", ability: "<id>" }` to cast an equipped class ability.
The non-ability actions are `loot`, `retreat`, and `idle`. Saving the file in an
external editor reloads it automatically; `R` remains available to force a
reload. The HUD shows the three latest script decisions with their input
values, output or error, reload number, and execution time. The runtime retains
the latest 32 decisions; press `T` to clear them. While stunned, the script
still runs and receives `hero.is_stunned == true`, but the game executes `idle`
until the stun expires.

Script authoring intentionally stays in an external editor. A failed `R` reload
or automatic reload reports the compile error while the last valid program
keeps running.

## MVP scope

- Fixed isometric camera and gray-box arena
- One programmable hero
- Enemy pursuit and melee attacks
- Content-driven classes, persistent ability loadouts, and generic ability casting
- Vanguard starter class with Strike, Cleave, Whirlwind, Charge, and Guard
- Unified timed status effects for poison, regeneration, stun, buffs, and debuffs
- Whirlwind stun/slow, Guard regeneration/defense, defensive retreat, and elite poison
- Data-driven enemy attack windups, range escapes, and whirlwind interrupts
- Weighted drop tables, level-scaled affixes, automatic pickup, bounded
  inventory, and stat-based equipment upgrades
- XP, levels, permanent build upgrades, and versioned save/load
- External-editor auto-reload, clear runtime errors, and bounded execution tracing
- Restartable, endlessly scaling waves across data-driven encounter layouts

The next systems milestone is continuous integration.

## Code structure

- `src/main.rs` only owns process startup and error reporting.
- `src/app.rs` is the composition root for focused plugins under `src/app/`.
  `gameplay.rs`, `effects.rs`, `arena.rs`, `controls.rs`, and `presentation.rs`
  own their systems independently, while `model.rs` contains their shared ECS
  types and ordered system sets. The gameplay, effects, and arena plugins run
  without rendering in headless integration tests.
- `src/gameplay.rs` contains deterministic game rules: balance values, health,
  typed damage results, cooldowns, abilities, nearest-target selection, bounded
  movement, and encounter phase resolution.
- `src/stats.rs` resolves base actor stats and flat/percentage modifiers for
  health, damage, defense, movement speed, and attack speed, including a
  separately replaceable temporary-modifier layer.
- `src/status_effects.rs` owns timed effects, refresh semantics, periodic
  damage/healing, stuns, and temporary stat modifiers.
- `src/items.rs` owns persistent item instances, rarity, bounded inventory,
  weapon/armor/boots equipment slots, stat-based upgrade rules, and aggregation
  of equipped modifiers.
- `src/progression.rs` owns XP thresholds, levels, upgrade spending, permanent
  stat bonuses, persisted class/loadout selection, save validation, and atomic
  profile writes.
- `src/scripting.rs` owns the versioned Rhai snapshot, command validation,
  sandbox limits, reload behavior, user-facing script errors, and bounded
  decision traces.
- `assets/content.ron` defines classes, abilities, base items, affixes, weighted
  drop tables, enemy archetypes, reusable spawn groups, the repeating encounter
  cycle, wave scaling, and optional enemy on-hit effects. Content references
  and combat values are validated at startup by `src/content.rs`.
- Hits flow through `DealDamage` and `DamageResolved` messages so future
  defenses, critical hits, effects, and combat logs have one integration point.
- `assets/hero.rhai` is the player-authored hero behavior.
- `src/tests/` keeps gameplay, status-effect, item, content, progression, stat,
  and scripting tests in separate files.

Keep new combat math in `gameplay.rs` when it can be expressed without querying
the Bevy world. This keeps the base systems fast to test and lets presentation
change without changing game rules.

Run the gameplay and scripting tests with:

```bash
cargo test
```

## Classes and abilities

Classes and abilities live in `assets/content.ron`. A class declares the
ability IDs available to its loadout; each ability declares its targeting mode,
damage scaling, range, cooldown, movement multiplier, and target/self status
effects. The generic runtime currently supports nearest-enemy, area (with an
optional target cap and approach behavior), and self-cast abilities.

The initial Vanguard loadout contains Strike, Cleave, Whirlwind, Charge, and
Guard. The selected class and ordered loadout are stored in the profile, so new
classes and a future loadout-selection UI do not require new combat action
branches.

## Encounter content

Item definitions, affixes, weighted drop tables, enemy stats, attack behavior,
spawn positions, and encounter composition live in `assets/content.ron`.
Enemies reference a drop table. A dropped item receives the current wave's item
level, level-scaled stats, and the number of compatible affixes allowed by its
rarity. Collected items remain in the hero inventory, and the strongest rolled
item found for each compatible slot is equipped automatically. Inventory holds
24 items; additional drops are salvaged for half their normal score so a full
inventory cannot stall the endless encounter loop.

Add an enemy archetype, reference it from a reusable spawn group, then reference
that group from an encounter. `encounter_sequence` defines the repeating layout
cycle for an endless run, while `wave_scaling` controls the per-wave increases
to enemy stats and XP. The next wave begins after every enemy is defeated and
its loot is collected; the hero keeps current health, inventory, and equipment
between waves. Content and cross-references are validated when the game starts,
so restart the game after editing this file.

## Progression saves

Defeated enemies award the XP declared by their archetype. Leveling grants one
upgrade point, which can be spent on Power, Vitality, or Swiftness. Permanent
upgrade modifiers stack with equipped-item modifiers. Inventory, equipped
items, XP, and upgrades all remain active when the arena restarts and are stored
in the versioned profile. Press `S` to save and `L` to load
`saves/profile.ron`; runtime save files are intentionally ignored by Git. Older
version-one progression saves are migrated with an empty inventory, and
version-two item IDs are migrated to level-one, unaffixed item instances.
Version-three profiles are migrated to the default Vanguard class and loadout.
The HUD always shows whether the current profile is saved, has unsaved changes,
has never been saved, or encountered a file error. Loading never replaces
unsaved progress without a second `L` press within three seconds, and failed
loads keep the current in-memory run intact.
