//! The mods: Luau scripts, one folder per mod with its `mod.toml` and its entry script, run by
//! the server alone. `shared::mods` finds the mods and orders them; this module runs them. A
//! mod reads the world through what the server hands it and changes it only through actions,
//! which the server applies once the mod has returned. The clients see their effects as they
//! see any other change of the world.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy::math::{IVec3, Vec3};
use bevy::prelude::*;
use mlua::{AppDataRefMut, Function, Lua, Table};
use shared::commands::COMMANDS;
use shared::mods::{plan_loading, FoundMod, LoadPlan, ModState, ModsSettings};
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
    /// What each mod handed to the mods that need it, by id.
    exports: HashMap<String, Table>,
    /// The mod whose script is running as it loads, and the mods it may import.
    loading: Option<Loading>,
}

/// A mod that loads.
struct Loading {
    id: String,
    /// Its dependencies, required or optional.
    may_import: HashSet<String>,
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
        api.set("export", lua.create_function(export)?)?;
        api.set("import", lua.create_function(import)?)?;
        let globals = lua.globals();
        globals.set("rustcraft", api)?;
        // mlua gives Luau a `require` that loads any script of the disk, sandbox or not
        globals.set("require", mlua::Nil)?;
        lua.sandbox(true)?;
        Ok(Mods { lua })
    }

    /// Loads the mods of `folder` that `settings` leaves on, as `load_mods` does.
    #[cfg(test)]
    pub fn load_folder(&self, folder: &Path, settings: &ModsSettings) -> LoadPlan {
        self.load_mods(&shared::mods::find_mods(folder), settings)
    }

    /// Loads the mods of `mods` that `settings` leaves on, dependencies first, else by id, and
    /// returns that plan. A mod that cannot load is skipped, and so are the mods that need it.
    pub fn load_mods(&self, mods: &[FoundMod], settings: &ModsSettings) -> LoadPlan {
        let plan = plan_loading(mods, settings);
        for (found, state) in mods.iter().zip(&plan.states) {
            match state {
                ModState::On => (),
                ModState::Off => info!("The mod {} is off", found.id),
                ModState::Broken(problem) => {
                    warn!("Could not load the mod {}: {problem}", found.id)
                }
                ModState::Waiting(needs) => {
                    let needs: Vec<String> = needs.iter().map(ToString::to_string).collect();
                    warn!("The mod {} needs {}", found.id, needs.join("; "));
                }
            }
        }
        let mut failed = HashSet::new();
        for found in plan.order.iter().map(|&index| &mods[index]) {
            let failed_dependency = found
                .manifest
                .dependencies
                .keys()
                .find(|id| failed.contains(id.as_str()));
            if let Some(dependency) = failed_dependency {
                warn!("The mod {} needs {dependency}, which failed", found.id);
                failed.insert(found.id.as_str());
                continue;
            }
            match self.load_mod(found) {
                Ok(()) => info!("Loaded the mod {} {}", found.id, found.version()),
                Err(err) => {
                    warn!("Could not load the mod {}: {err}", found.id);
                    failed.insert(found.id.as_str());
                }
            }
        }
        plan
    }

    /// Runs the entry script of `found`, if it has one. While it runs, the script may import
    /// what its dependencies exported; what it exports itself goes if it fails.
    fn load_mod(&self, found: &FoundMod) -> mlua::Result<()> {
        let Some(entry) = found.entry() else {
            return Ok(());
        };
        let source = std::fs::read_to_string(&entry).map_err(|err| {
            mlua::Error::runtime(format!("cannot read {}: {err}", entry.display()))
        })?;
        let manifest = &found.manifest;
        self.host().loading = Some(Loading {
            id: found.id.clone(),
            may_import: manifest
                .dependencies
                .keys()
                .chain(manifest.optional.keys())
                .cloned()
                .collect(),
        });
        let result = self.load(&script_name(found, &entry), &source);
        let mut host = self.host();
        host.loading = None;
        if result.is_err() {
            host.exports.remove(&found.id);
        }
        result
    }

    /// Runs the script called `name` in the errors, `<mod>/<path>`. A script that fails takes
    /// back the commands it registered.
    pub fn load(&self, name: &str, source: &str) -> mlua::Result<()> {
        let before: HashSet<String> = self.host().commands.keys().cloned().collect();
        let result = self.lua.load(source).set_name(format!("@{name}")).exec();
        if result.is_err() {
            self.host()
                .commands
                .retain(|command, _| before.contains(command));
        }
        result
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

/// The name of the script `path` of `found` in the errors: `<mod>/<path in its folder>`.
fn script_name(found: &FoundMod, path: &Path) -> String {
    let script = path.strip_prefix(&found.folder).unwrap_or(path);
    format!("{}/{}", found.id, script.display())
}

/// Compiles the entry script of `found` without running it: the error of a script that does
/// not parse, as the Mods screen shows it. `None` when it parses or the mod has no code.
pub fn script_error(found: &FoundMod) -> Option<String> {
    let entry = found.entry()?;
    let source = match std::fs::read_to_string(&entry) {
        Ok(source) => source,
        Err(err) => return Some(format!("Cannot read {}: {err}", script_name(found, &entry))),
    };
    Lua::new()
        .load(source)
        .set_name(format!("@{}", script_name(found, &entry)))
        .into_function()
        .err()
        .map(|err| err.to_string())
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

/// `rustcraft.export(table)`: hands `table` to the mods that need this one, which get it from
/// `rustcraft.import`. Only while the mod loads; a second call replaces the first.
fn export(lua: &Lua, table: Table) -> mlua::Result<()> {
    let mut host: AppDataRefMut<Host> = lua.app_data_mut().expect("the host is set");
    let Some(loading) = &host.loading else {
        return Err(mlua::Error::runtime(
            "rustcraft.export works only while the mod loads",
        ));
    };
    let id = loading.id.clone();
    host.exports.insert(id, table);
    Ok(())
}

/// `rustcraft.import(id)`: what the mod `id` exported, or `nil` when it is not loaded, as an
/// optional dependency that is off. Only while the mod loads, and only from a mod its
/// `mod.toml` names, so that the order of loading guarantees it ran first.
fn import(lua: &Lua, id: String) -> mlua::Result<Option<Table>> {
    let host = lua.app_data_ref::<Host>().expect("the host is set");
    let Some(loading) = &host.loading else {
        return Err(mlua::Error::runtime(
            "rustcraft.import works only while the mod loads",
        ));
    };
    if !loading.may_import.contains(&id) {
        return Err(mlua::Error::runtime(format!(
            "{id} is not a dependency of {}: name it in its mod.toml",
            loading.id
        )));
    }
    Ok(host.exports.get(&id).cloned())
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
        write(folder.path(), "tower/init.luau", TOWER);
        write(folder.path(), "broken/init.luau", "this is not luau");
        let mods = Mods::new().unwrap();

        mods.load_folder(folder.path(), &ModsSettings::default());

        let answer = mods.run_command("tower 1", "steve", &mut world_with_steve());
        assert_eq!(answer.as_deref(), Some("Tower of 1 blocks"));
        assert_eq!(mods.command_names(), ["tower"]);
    }

    fn write(folder: &Path, path: &str, text: &str) {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A manifest of the mod `id`, whose script registers the command `/<id>`, answering with
    /// the answer of `/<needs>` when it needs a mod.
    fn command_mod(folder: &Path, id: &str, needs: Option<&str>) {
        let dependencies = needs.map_or(String::new(), |needs| format!("{needs} = \"*\""));
        write(
            folder,
            &format!("{id}/mod.toml"),
            &format!(
                "[mod]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\nentry = \"main.luau\"\n\
                 [game]\napi = 1\n[dependencies]\n{dependencies}\n"
            ),
        );
        write(
            folder,
            &format!("{id}/main.luau"),
            &format!("rustcraft.register_command(\"{id}\", function() return \"{id}\" end)"),
        );
    }

    #[test]
    fn the_mods_turned_off_and_those_that_need_them_do_not_load() {
        let folder = tempfile::tempdir().unwrap();
        command_mod(folder.path(), "base", None);
        command_mod(folder.path(), "addon", Some("base"));
        command_mod(folder.path(), "other", None);
        let mut settings = ModsSettings::default();
        settings.set_on("base", false);
        let mods = Mods::new().unwrap();

        mods.load_folder(folder.path(), &settings);

        assert_eq!(mods.command_names(), ["other"]);
    }

    #[test]
    fn a_mod_whose_dependency_fails_does_not_load() {
        let folder = tempfile::tempdir().unwrap();
        command_mod(folder.path(), "base", None);
        command_mod(folder.path(), "addon", Some("base"));
        write(folder.path(), "base/main.luau", "error('no')");
        let mods = Mods::new().unwrap();

        mods.load_folder(folder.path(), &ModsSettings::default());

        assert!(mods.command_names().is_empty());
    }

    #[test]
    fn a_failing_script_takes_back_its_commands() {
        let mods = Mods::new().unwrap();
        let script = r#"
            rustcraft.register_command("first", function() end)
            error("broken after the first command")
        "#;
        assert!(mods.load("half/init.luau", script).is_err());
        assert!(mods.command_names().is_empty());
    }

    #[test]
    fn a_script_that_does_not_parse_says_where_without_running() {
        let folder = tempfile::tempdir().unwrap();
        write(
            folder.path(),
            "good/init.luau",
            "error('runs only in the game')",
        );
        write(folder.path(), "bad/init.luau", "local x = = 1");
        write(folder.path(), "none/mod.toml", &shared_manifest("none"));
        let found = shared::mods::find_mods(folder.path());
        let error_of = |id: &str| script_error(found.iter().find(|m| m.id == id).unwrap());

        assert_eq!(error_of("good"), None);
        assert_eq!(error_of("none"), None);
        let error = error_of("bad").unwrap();
        assert!(error.contains("bad/init.luau:1"), "{error}");
    }

    fn shared_manifest(id: &str) -> String {
        format!("[mod]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\n[game]\napi = 1\n")
    }

    /// The mods of `examples/mods`: Tower, Shapes, Watchtower, which needs Shapes, and Beacon,
    /// which works with it.
    fn examples() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/mods")
    }

    #[test]
    fn the_example_mods_build_with_shapes_when_it_is_on() {
        let mods = Mods::new().unwrap();
        let plan = mods.load_folder(&examples(), &ModsSettings::default());
        assert!(plan.states.iter().all(|state| *state == ModState::On));
        assert_eq!(
            mods.command_names(),
            ["beacon", "pillar", "tower", "watchtower"]
        );

        // steve stands in (1, 2, 3): everything is built from (3, 2, 3)
        let mut world_map = world_with_steve();
        let answer = mods.run_command("watchtower", "steve", &mut world_map);
        assert_eq!(answer.as_deref(), Some("Watchtower built"));
        for (x, z) in [(3, 3), (6, 3), (3, 6), (6, 6)] {
            for y in 2..6 {
                assert_eq!(
                    block(&world_map, x, y, z),
                    Some(BlockId::OakLog),
                    "{x} {y} {z}"
                );
            }
        }
        assert_eq!(block(&world_map, 4, 6, 5), Some(BlockId::OakPlanks));

        let mut world_map = world_with_steve();
        let answer = mods.run_command("beacon", "steve", &mut world_map);
        assert_eq!(answer.as_deref(), Some("Beacon lit on a pillar"));
        assert_eq!(block(&world_map, 3, 4, 3), Some(BlockId::Cobblestone));
        assert_eq!(block(&world_map, 3, 5, 3), Some(BlockId::Torch));
    }

    #[test]
    fn without_shapes_watchtower_waits_and_beacon_does_without() {
        let mut settings = ModsSettings::default();
        settings.set_on("shapes", false);
        let mods = Mods::new().unwrap();
        mods.load_folder(&examples(), &settings);
        assert_eq!(mods.command_names(), ["beacon", "tower"]);

        let mut world_map = world_with_steve();
        let answer = mods.run_command("beacon", "steve", &mut world_map);
        assert_eq!(
            answer.as_deref(),
            Some("Beacon lit, on the ground without Shapes")
        );
        assert_eq!(block(&world_map, 3, 2, 3), Some(BlockId::Torch));
        assert_eq!(block(&world_map, 3, 3, 3), None);
    }

    #[test]
    fn a_mod_imports_only_its_dependencies_and_only_as_it_loads() {
        let folder = tempfile::tempdir().unwrap();
        write(folder.path(), "lib/mod.toml", &shared_manifest("lib"));
        write(
            folder.path(),
            "lib/init.luau",
            "rustcraft.export({ answer = 42 })",
        );
        write(
            folder.path(),
            "stranger/mod.toml",
            &shared_manifest("stranger"),
        );
        write(
            folder.path(),
            "stranger/init.luau",
            "rustcraft.register_command('stranger', function() end)\nrustcraft.import('lib')",
        );
        write(
            folder.path(),
            "late/mod.toml",
            &(shared_manifest("late") + "[dependencies]\nlib = \"*\"\n"),
        );
        write(
            folder.path(),
            "late/init.luau",
            "rustcraft.register_command('late', function() \
                return tostring(rustcraft.import('lib')) end)",
        );
        let mods = Mods::new().unwrap();
        mods.load_folder(folder.path(), &ModsSettings::default());

        // stranger did not name lib: its script failed and took back its command
        assert_eq!(mods.command_names(), ["late"]);
        let answer = mods.run_command("late", "steve", &mut world_with_steve());
        assert_eq!(answer.as_deref(), Some("The command /late failed"));
    }

    #[test]
    fn a_mod_that_fails_after_exporting_hands_nothing() {
        let folder = tempfile::tempdir().unwrap();
        write(folder.path(), "lib/mod.toml", &shared_manifest("lib"));
        write(
            folder.path(),
            "lib/init.luau",
            "rustcraft.export({})\nerror('broken')",
        );
        write(
            folder.path(),
            "user/mod.toml",
            &(shared_manifest("user") + "[optional]\nlib = \"*\"\n"),
        );
        write(
            folder.path(),
            "user/init.luau",
            "local lib = rustcraft.import('lib')\n\
             rustcraft.register_command('user', function() return tostring(lib == nil) end)",
        );
        let mods = Mods::new().unwrap();
        mods.load_folder(folder.path(), &ModsSettings::default());

        let answer = mods.run_command("user", "steve", &mut world_with_steve());
        assert_eq!(answer.as_deref(), Some("true"));
    }
}
