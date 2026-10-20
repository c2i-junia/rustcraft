-- `/tower <height>` builds a column of cobblestone two blocks east of its author.
-- To try it, copy this folder into the `mods` folder of the game folder: the Mods screen of the
-- title screen then turns it on or off.

local rc = rustcraft
rc.register_command("tower", function(ctx, args)
    local height = tonumber(args[1]) or 5
    local p = rc.player_position(ctx.player)
    for y = 0, height - 1 do
        rc.set_block({ x = p.x + 2, y = p.y + y, z = p.z }, "cobblestone")
    end
    return "Tower of " .. height .. " blocks"
end)
