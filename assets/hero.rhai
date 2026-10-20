// Edit this file while the game is running, then press R to reload it.
// API v1: decide receives a world snapshot and returns a command object.
// Valid actions: "attack", "whirlwind", "loot", "retreat", "idle".

fn decide(world) {
    if world.hero.health_percent < 0.25 {
        return #{ action: "retreat" };
    }

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
