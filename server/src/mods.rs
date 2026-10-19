//! The mods: Luau scripts, one folder per mod with its `init.luau`, run by the server alone. A
//! mod reads the world through what the server hands it and changes it only through actions,
//! which the server applies once the mod has returned. The clients see their effects as they
//! see any other change of the world.

use std::collections::HashMap;
use std::path::Path;

use bevy::math::{IVec3, Vec3};
use bevy::prelude::*;
use mlua::{AppDataRefMut, Function, Lua, Table};
use shared::commands::COMMANDS;
use shared::world::{BlockData, BlockDirection, BlockId, ItemId, ItemType};

use crate::world::data::ServerWorldMap;

/// What a mod asks the server to do.
#[derive(Debug, PartialEq)]
pub enum ModAction {
    SetBlock(IVec3, BlockId),
}

/// What the functions given to the mods share with the server.
#[derive(Default)]
struct Host {
    /// The chat commands of the mods, by name.
    commands: HashMap<String, Function>,
    /// The position of each player, by name, as the world was when the server called the mod.
    positions: HashMap<String, Vec3>,
    /// The actions asked by the mod being called.
    actions: Vec<ModAction>,
}

/// The Luau VM that runs every mod, sandboxed: no `io`, no `require`, read-only libraries and
/// globals, and a global table of its own for each mod.
#[derive(Resource)]
pub struct Mods {
    lua: Lua,
}

impl Mods {
    pub fn new() -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.set_app_data(Host::default());
        let api = lua.create_table()?;
        api.set("register_command", lua.create_function(register_command)?)?;
        api.set("player_position", lua.create_function(player_position)?)?;
        api.set("set_block", lua.create_function(set_block)?)?;
        let globals = lua.globals();
        globals.set("rustcraft", api)?;
        // mlua gives Luau a `require` that loads any script of the disk, sandbox or not
        globals.set("require", mlua::Nil)?;
        lua.sandbox(true)?;
        Ok(Mods { lua })
    }

    /// Loads every mod of `folder`, in the order of their names: each subfolder with an
    /// `init.luau` is one. A mod that fails to load is skipped.
    pub fn load_folder(&self, folder: &Path) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        let mut scripts: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().join("init.luau"))
            .filter(|script| script.is_file())
            .collect();
        scripts.sort();
        for script in scripts {
            let name = script
                .parent()
                .and_then(Path::file_name)
                .unwrap_or_default();
            let name = name.to_string_lossy();
            let loaded = std::fs::read_to_string(&script)
                .map_err(mlua::Error::external)
                .and_then(|source| self.load(&name, &source));
            match loaded {
                Ok(()) => info!("Loaded mod {name}"),
                Err(err) => warn!("Could not load mod {name}: {err}"),
            }
        }
    }

    /// Runs the `init.luau` of the mod `name`.
    pub fn load(&self, name: &str, source: &str) -> mlua::Result<()> {
        self.lua
            .load(source)
            .set_name(format!("@{name}/init.luau"))
            .exec()
    }

    /// Runs the mod command of `line`, a command line without its `/`, for the player called
    /// `author`, and applies what it asked. Returns the answer for the author, empty when the
    /// mod gave none, or `None` when no mod has this command. A command that fails changes
    /// nothing.
    pub fn run_command(
        &self,
        line: &str,
        author: &str,
        world_map: &mut ServerWorldMap,
    ) -> Option<String> {
        let mut words = line.split_whitespace();
        let name = words.next()?;
        let function = self.host().commands.get(name)?.clone();
        let args: Vec<String> = words.map(String::from).collect();

        self.host().positions = world_map
            .players
            .values()
            .map(|player| (player.name.clone(), player.position))
            .collect();
        let result = self
            .lua
            .create_table_from([("player", author)])
            .and_then(|ctx| function.call::<Option<String>>((ctx, args)));
        let actions = std::mem::take(&mut self.host().actions);

        match result {
            Ok(answer) => {
                apply(actions, world_map);
                Some(answer.unwrap_or_default())
            }
            Err(err) => {
                warn!("The mod command /{name} failed: {err}");
                Some(format!("The command /{name} failed"))
            }
        }
    }

    /// The names of the commands of the mods, in alphabetical order.
    pub fn command_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.host().commands.keys().cloned().collect();
        names.sort();
        names
    }

    fn host(&self) -> AppDataRefMut<'_, Host> {
        self.lua.app_data_mut().expect("the host is set by `new`")
    }
}

fn apply(actions: Vec<ModAction>, world_map: &mut ServerWorldMap) {
    for action in actions {
        match action {
            ModAction::SetBlock(pos, id) => world_map
                .chunks
                .set_block(pos, BlockData::new(id, BlockDirection::Front)),
        }
    }
}

/// `rustcraft.register_command(name, function(ctx, args))`: `/name` runs the function with
/// `ctx.player`, the name of its author, and the words after the name. The string it returns
/// is the answer to the author.
fn register_command(lua: &Lua, (name, function): (String, Function)) -> mlua::Result<()> {
    if COMMANDS.iter().any(|command| command.name == name) {
        return Err(mlua::Error::runtime(format!(
            "/{name} is a command of the game"
        )));
    }
    let mut host: AppDataRefMut<Host> = lua.app_data_mut().expect("the host is set");
    host.commands.insert(name, function);
    Ok(())
}

/// `rustcraft.player_position(name)`: the block `{x, y, z}` the player is in, or `nil` when
/// no player has this name.
fn player_position(lua: &Lua, name: String) -> mlua::Result<Option<Table>> {
    let position = lua
        .app_data_ref::<Host>()
        .expect("the host is set")
        .positions
        .get(&name)
        .map(|position| position.floor().as_ivec3());
    position
        .map(|pos| lua.create_table_from([("x", pos.x), ("y", pos.y), ("z", pos.z)]))
        .transpose()
}

/// `rustcraft.set_block({x, y, z}, name)`: puts the block called `name`, like `oak_log`, at
/// this position. Nothing happens where the world is not loaded.
fn set_block(lua: &Lua, (pos, name): (Table, String)) -> mlua::Result<()> {
    let coordinate = |axis: &str| pos.get::<f64>(axis).map(|value| value.floor() as i32);
    let pos = IVec3::new(coordinate("x")?, coordinate("y")?, coordinate("z")?);
    let Some(ItemType::Block(id)) = ItemId::from_name(&name).map(|item| item.get_default_type())
    else {
        return Err(mlua::Error::runtime(format!("Unknown block: {name}")));
    };
    let mut host: AppDataRefMut<Host> = lua.app_data_mut().expect("the host is set");
    host.actions.push(ModAction::SetBlock(pos, id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Transform;
    use shared::players::Player;
    use shared::world::ChunkMap;

    const TOWER: &str = r#"
        local rc = rustcraft
        rc.register_command("tower", function(ctx, args)
            local height = tonumber(args[1]) or 5
            local p = rc.player_position(ctx.player)
            for y = 0, height - 1 do
                rc.set_block({ x = p.x + 2, y = p.y + y, z = p.z }, "cobblestone")
            end
            return "Tower of " .. height .. " blocks"
        end)
    "#;

    fn world_with_steve() -> ServerWorldMap {
        let mut world_map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::new(0, 0, 0), IVec3::new(8, 8, 8)),
            ..Default::default()
        };
        let steve = Player::new(
            1,
            "steve".into(),
            Vec3::new(1.5, 2.2, 3.5),
            Transform::default(),
        );
        world_map.players.insert(1, steve);
        world_map
    }

    fn block(world_map: &ServerWorldMap, x: i32, y: i32, z: i32) -> Option<BlockId> {
        world_map
            .chunks
            .get_block(IVec3::new(x, y, z))
            .map(|block| block.id)
    }

    #[test]
    fn a_mod_command_builds_next_to_its_author() {
        let mods = Mods::new().unwrap();
        mods.load("tower", TOWER).unwrap();
        let mut world_map = world_with_steve();

        let answer = mods.run_command("tower 3", "steve", &mut world_map);

        assert_eq!(answer.as_deref(), Some("Tower of 3 blocks"));
        for y in 2..5 {
            assert_eq!(block(&world_map, 3, y, 3), Some(BlockId::Cobblestone));
        }
        assert_eq!(block(&world_map, 3, 5, 3), None);
    }

    #[test]
    fn a_command_no_mod_has_is_left_to_the_game() {
        let mods = Mods::new().unwrap();
        mods.load("tower", TOWER).unwrap();
        assert_eq!(
            mods.run_command("castle", "steve", &mut world_with_steve()),
            None
        );
    }

    #[test]
    fn a_failing_command_changes_nothing() {
        let mods = Mods::new().unwrap();
        let script = r#"
            rustcraft.register_command("half", function(ctx, args)
                rustcraft.set_block({ x = 1, y = 1, z = 1 }, "stone")
                rustcraft.set_block({ x = 1, y = 2, z = 1 }, "no_such_block")
            end)
        "#;
        mods.load("half", script).unwrap();
        let mut world_map = world_with_steve();

        let answer = mods.run_command("half", "steve", &mut world_map);

        assert_eq!(answer.as_deref(), Some("The command /half failed"));
        assert_eq!(block(&world_map, 1, 1, 1), None);
    }

    #[test]
    fn a_mod_cannot_take_a_command_of_the_game() {
        let mods = Mods::new().unwrap();
        let script = r#"rustcraft.register_command("give", function() end)"#;
        assert!(mods.load("thief", script).is_err());
    }

    #[test]
    fn a_mod_cannot_reach_the_files_nor_change_the_api() {
        let mods = Mods::new().unwrap();
        mods.load(
            "probe",
            "assert(io == nil and require == nil and loadfile == nil)",
        )
        .unwrap();
        assert!(mods.load("vandal", "rustcraft.set_block = nil").is_err());
    }

    #[test]
    fn mods_are_loaded_from_their_folders() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::create_dir(folder.path().join("tower")).unwrap();
        std::fs::write(folder.path().join("tower/init.luau"), TOWER).unwrap();
        std::fs::create_dir(folder.path().join("broken")).unwrap();
        std::fs::write(folder.path().join("broken/init.luau"), "this is not luau").unwrap();
        let mods = Mods::new().unwrap();

        mods.load_folder(folder.path());

        let answer = mods.run_command("tower 1", "steve", &mut world_with_steve());
        assert_eq!(answer.as_deref(), Some("Tower of 1 blocks"));
        assert_eq!(mods.command_names(), ["tower"]);
    }
}
