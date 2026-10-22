-- A test mod that works with Shapes, an optional dependency of its mod.toml: rustcraft.import
-- gives nil when Shapes is missing or off, and the torch then stands on the ground.

local rc = rustcraft
local shapes = rc.import("shapes")

rc.register_command("beacon", function(ctx)
    local p = rc.player_position(ctx.player)
    local base = { x = p.x + 2, y = p.y, z = p.z }
    if shapes then
        shapes.pillar(base, 3, "cobblestone")
        rc.set_block({ x = base.x, y = base.y + 3, z = base.z }, "torch")
        return "Beacon lit on a pillar"
    end
    rc.set_block(base, "torch")
    return "Beacon lit, on the ground without Shapes"
end)
