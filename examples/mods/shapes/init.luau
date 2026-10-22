-- A test mod that needs no other. It hands `pillar` and `floor` to the mods that need it,
-- through rustcraft.export, and builds a pillar itself with `/pillar <height>`.
-- To try it, copy this folder into the `mods` folder of the game folder.

local rc = rustcraft
local shapes = {}

-- A column of `height` blocks of `block`, from `p` up.
function shapes.pillar(p, height, block)
    for y = 0, height - 1 do
        rc.set_block({ x = p.x, y = p.y + y, z = p.z }, block)
    end
end

-- A square of `side` blocks of `block`, level with `p`, `p` at its corner.
function shapes.floor(p, side, block)
    for x = 0, side - 1 do
        for z = 0, side - 1 do
            rc.set_block({ x = p.x + x, y = p.y, z = p.z + z }, block)
        end
    end
end

rc.export(shapes)

rc.register_command("pillar", function(ctx, args)
    local height = tonumber(args[1]) or 3
    local p = rc.player_position(ctx.player)
    shapes.pillar({ x = p.x + 2, y = p.y, z = p.z }, height, "cobblestone")
    return "Pillar of " .. height .. " blocks"
end)
