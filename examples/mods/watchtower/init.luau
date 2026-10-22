-- A test mod that needs Shapes, which its mod.toml names: Shapes is loaded first, so its
-- builders are there to import.

local rc = rustcraft
local shapes = rc.import("shapes")

local SIDE = 4
local HEIGHT = 4

rc.register_command("watchtower", function(ctx)
    local p = rc.player_position(ctx.player)
    local corner = { x = p.x + 2, y = p.y, z = p.z }
    for _, offset in ipairs({ { 0, 0 }, { SIDE - 1, 0 }, { 0, SIDE - 1 }, { SIDE - 1, SIDE - 1 } }) do
        shapes.pillar({ x = corner.x + offset[1], y = corner.y, z = corner.z + offset[2] }, HEIGHT, "oak_log")
    end
    shapes.floor({ x = corner.x, y = corner.y + HEIGHT, z = corner.z }, SIDE, "oak_planks")
    return "Watchtower built"
end)
