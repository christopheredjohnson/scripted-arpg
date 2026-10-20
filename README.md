# Scripted ARPG MVP

A code-first 3D combat sandbox built with Rust, Bevy, and Rhai. The hero is controlled by the program in `assets/hero.rhai` rather than direct movement input.

## Run

```bash
cargo run
```

The first build is large because Bevy and its rendering stack must compile.

## Controls

- `R`: reload `assets/hero.rhai`
- `Space`: restart the arena
- `1`: spend an upgrade point on Power
- `2`: spend an upgrade point on Vitality
- `3`: spend an upgrade point on Swiftness
- `S`: save progression to `saves/profile.ron`
- `L`: load saved progression
- `Escape`: quit

## Hero program

The game calls this API v1 function five times per second. It receives a world
snapshot and returns a validated command object:

```rhai
fn decide(world) {
    if world.encounter.enemy_count >= 3
        && world.abilities.whirlwind_ready
        && world.encounter.nearest_enemy_distance < 3.5
    {
        return #{ action: "whirlwind" };
    }
    if world.encounter.enemy_count > 0 {
        return #{ action: "attack" };
    }
    if world.encounter.loot_nearby {
        return #{ action: "loot" };
    }
    #{ action: "idle" }
}
```

The snapshot contains `api_version`, `hero.health_percent`,
`encounter.enemy_count`, `encounter.nearest_enemy_distance`,
`encounter.loot_nearby`, and `abilities.whirlwind_ready`. Valid actions are
`attack`, `whirlwind`, `loot`, `retreat`, and `idle`. Save the file and press
`R` in the running game to apply changes.

## MVP scope

- Fixed isometric camera and gray-box arena
- One programmable hero
- Enemy pursuit and melee attacks
- Basic attack and area whirlwind ability
- Defensive retreat with temporary health recovery
- Data-driven item drops, automatic pickup, inventory, and equipment
- XP, levels, permanent build upgrades, and versioned save/load
- Script reload and clear runtime errors
- Restartable combat encounter

The next milestone is an in-game editor with line highlighting and execution tracing.

## Code structure

- `src/gameplay.rs` contains deterministic game rules: balance values, health,
  typed damage results, cooldowns, abilities, nearest-target selection, bounded
  movement, and encounter phase resolution.
- `src/stats.rs` resolves base actor stats and flat/percentage modifiers for
  health, damage, defense, movement speed, and attack speed.
- `src/items.rs` owns item rarity, inventory, weapon/armor/boots equipment
  slots, upgrade rules, and aggregation of equipped stat modifiers.
- `src/progression.rs` owns XP thresholds, levels, upgrade spending, permanent
  stat bonuses, save validation, and atomic profile writes.
- `src/scripting.rs` owns the versioned Rhai snapshot, command validation,
  sandbox limits, reload behavior, and user-facing script errors.
- `assets/content.ron` defines items, enemy archetypes, reusable spawn groups,
  and the starting encounter. Content references and combat values are
  validated at startup by `src/content.rs`.
- `src/main.rs` connects those rules to Bevy entities, rendering, input, and the
  Rhai scripting boundary. Hits flow through `DealDamage` and `DamageResolved`
  messages so future defenses, critical hits, effects, and combat logs have one
  integration point.
- `assets/hero.rhai` is the player-authored hero behavior.
- `src/tests/` keeps gameplay, item, content, progression, stat, and scripting
  tests in separate files.

Keep new combat math in `gameplay.rs` when it can be expressed without querying
the Bevy world. This keeps the base systems fast to test and lets presentation
change without changing game rules.

Run the gameplay and scripting tests with:

```bash
cargo test
```

## Encounter content

Item definitions, enemy stats, attack behavior, item drops, spawn positions,
and encounter composition live in `assets/content.ron`. Items declare an
equipment slot and any flat or percentage stat modifiers. Enemies reference an
item ID for their drop. Collected items remain in the hero inventory, and the
highest-rarity item found for each compatible slot is equipped automatically.

Add an enemy archetype, reference it from a reusable spawn group, then reference
that group from an encounter. Set `starting_encounter` to choose which encounter
launches. Content and cross-references are validated when the game starts, so
restart the game after editing this file.

## Progression saves

Defeated enemies award the XP declared by their archetype. Leveling grants one
upgrade point, which can be spent on Power, Vitality, or Swiftness. Permanent
upgrade modifiers stack with equipped-item modifiers and remain active when the
arena restarts. Press `S` to save and `L` to load the versioned profile at
`saves/profile.ron`; runtime save files are intentionally ignored by Git.
