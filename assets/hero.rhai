// Edit this file while the game is running; saved changes reload automatically.
// API v3: decide receives a world snapshot and returns a command object.
// Cast a class ability with #{ action: "cast", ability: "ability_id" }.

fn decide(world) {
    // Start by tuning these values to change the hero's personality.
    let guard_below = 0.35;
    let retreat_below = 0.22;
    let whirlwind_targets = 3;
    let interrupt_windups = true;

    // Stunned heroes cannot execute an action, but returning idle makes the
    // decision trace accurately describe the intent.
    if world.hero.is_stunned {
        return #{ action: "idle" };
    }

    // Retreat cannot create space when a pack already surrounds the hero.
    // Fight that pack so combat always progresses instead of refreshing the
    // retreat defenses forever.
    let surrounded = world.abilities.whirlwind.targets_in_range >= whirlwind_targets;
    let can_interrupt = interrupt_windups
        && world.abilities.whirlwind.ready
        && world.abilities.whirlwind.winding_up_targets_in_range > 0;
    if can_interrupt {
        return #{ action: "cast", ability: "whirlwind" };
    }

    if surrounded {
        if world.abilities.whirlwind.ready {
            return #{ action: "cast", ability: "whirlwind" };
        }
        if world.abilities.cleave.ready {
            return #{ action: "cast", ability: "cleave" };
        }
        return #{ action: "cast", ability: "strike" };
    }

    // Guard buys time with regeneration and defense instead of immediately
    // entering a retreat loop whenever health falls.
    if world.encounter.enemy_count > 0
        && world.hero.health_percent <= guard_below
        && world.abilities.guard.ready
    {
        return #{ action: "cast", ability: "guard" };
    }

    // Retreat only when there is room to escape. Checking for enemies also
    // avoids running away from loot after the last enemy dies.
    if world.encounter.enemy_count > 0
        && world.hero.health_percent <= retreat_below
    {
        return #{ action: "retreat" };
    }

    // Basic attack also moves toward the nearest living enemy.
    if world.encounter.enemy_count > 0 {
        if world.encounter.nearest_enemy_distance > world.abilities.cleave.range
            && world.abilities.charge.ready
        {
            return #{ action: "cast", ability: "charge" };
        }
        if world.encounter.enemy_count >= 2
            && world.encounter.nearest_enemy_distance <= world.abilities.cleave.range
            && world.abilities.cleave.ready
        {
            return #{ action: "cast", ability: "cleave" };
        }
        return #{ action: "cast", ability: "strike" };
    }

    // Loot moves toward the nearest drop, even when it is not nearby yet.
    if world.encounter.loot_count > 0 {
        return #{ action: "loot" };
    }

    #{ action: "idle" }
}
