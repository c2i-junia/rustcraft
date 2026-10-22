//! The mods of a game folder: each folder of `mods/` with a `mod.toml`, or with only an
//! `init.luau`, as before manifests existed. This module reads them, takes out those that
//! `mods.ron` turns off, and puts the others in the order the server loads them: dependencies
//! first, then by id. It runs no code: the server runs the scripts, and the Mods screen of the
//! client shows what it found.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::world::registry::{base_mod_folder, BASE_MOD};

/// The folder of the mods, in the game folder.
pub const MODS_FOLDER: &str = "mods";
/// The mods the player turned off, in the game folder.
pub const MODS_SETTINGS_PATH: &str = "mods.ron";
/// The declaration of a mod, in its folder.
pub const MANIFEST_FILE: &str = "mod.toml";
/// The script run when a mod loads, unless its manifest names another.
pub const DEFAULT_ENTRY: &str = "init.luau";
/// The icon of a mod, unless its manifest names another.
pub const DEFAULT_ICON: &str = "icon.png";
/// The version of the `rustcraft` table of the scripts. A mod written for a later one is not
/// loaded.
pub const API_VERSION: u32 = 1;
pub const MAX_ID_LENGTH: usize = 32;
pub const MAX_NAME_LENGTH: usize = 40;

/// What `mod.toml` declares. Keys it does not know are left out, so that a manifest written
/// for a later game, with blocks of its own, still reads.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Manifest {
    #[serde(rename = "mod")]
    pub about: About,
    pub game: GameRequirement,
    /// The mods loaded before this one, which it cannot load without.
    #[serde(default)]
    pub dependencies: BTreeMap<String, VersionReq>,
    /// The mods loaded before this one when they are there and on.
    #[serde(default)]
    pub optional: BTreeMap<String, VersionReq>,
}

/// The `[mod]` table of a manifest.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct About {
    pub id: String,
    pub name: String,
    pub version: Version,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub license: Option<String>,
    /// A path in the folder of the mod, `DEFAULT_ICON` when absent.
    pub icon: Option<String>,
    pub homepage: Option<String>,
    /// A path in the folder of the mod, `DEFAULT_ENTRY` when absent.
    pub entry: Option<String>,
}

/// The `[game]` table of a manifest.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct GameRequirement {
    /// The versions of the game the mod runs on. Read but not checked yet: every build of the
    /// game is version 0.0.0.
    pub version: Option<VersionReq>,
    /// The version of the `rustcraft` table the mod was written for.
    pub api: u32,
}

/// A mod found in the folder of the mods.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundMod {
    /// The name of its folder.
    pub id: String,
    pub folder: PathBuf,
    /// What its manifest declares, or, for a folder without one or with one that cannot be
    /// read, a manifest named after the folder.
    pub manifest: Manifest,
    /// Whether the folder has a `mod.toml`.
    pub declared: bool,
    /// Whether it comes with the game, as the base mod, rather than from `mods/`.
    pub shipped: bool,
    /// Why the mod cannot load, whatever the settings: a manifest that cannot be read or that
    /// breaks a rule.
    pub problem: Option<String>,
}

impl FoundMod {
    /// A mod named after its folder, as one without a manifest is: version 0.0.0, written for
    /// the current API.
    fn undeclared(id: &str, folder: PathBuf, problem: Option<String>) -> Self {
        FoundMod {
            id: id.to_string(),
            folder,
            manifest: Manifest {
                about: About {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: Version::new(0, 0, 0),
                    description: String::new(),
                    authors: Vec::new(),
                    license: None,
                    icon: None,
                    homepage: None,
                    entry: None,
                },
                game: GameRequirement {
                    version: None,
                    api: API_VERSION,
                },
                dependencies: BTreeMap::new(),
                optional: BTreeMap::new(),
            },
            declared: false,
            shipped: false,
            problem,
        }
    }

    pub fn name(&self) -> &str {
        &self.manifest.about.name
    }

    pub fn version(&self) -> &Version {
        &self.manifest.about.version
    }

    /// The script to run when the mod loads: the one its manifest names, which must then
    /// exist, else `init.luau` when there is one. `None` for a mod without code.
    pub fn entry(&self) -> Option<PathBuf> {
        match &self.manifest.about.entry {
            Some(entry) => Some(self.folder.join(entry)),
            None => Some(self.folder.join(DEFAULT_ENTRY)).filter(|path| path.is_file()),
        }
    }

    /// The icon of the mod, if its file is there.
    pub fn icon(&self) -> Option<PathBuf> {
        let icon = self.manifest.about.icon.as_deref().unwrap_or(DEFAULT_ICON);
        Some(self.folder.join(icon)).filter(|path| path.is_file())
    }
}

/// The mods a game folder runs, by id: the base mod, which comes with the game (see
/// `world::registry`), and those of its `mods/`. A mod of `mods/` that takes the base mod's id
/// cannot load.
pub fn find_game_mods(game_folder: &Path) -> Vec<FoundMod> {
    let mut mods = find_mods(&game_folder.join(MODS_FOLDER));
    for found in mods.iter_mut().filter(|found| found.id == BASE_MOD) {
        found.problem = Some(format!(
            "{BASE_MOD} is the id of the game's own mod: rename this folder and its id"
        ));
        // apart from the base mod: two mods of one id would mix in the plan
        found.id = format!("{BASE_MOD} ({MODS_FOLDER}/)");
    }
    if let Some(mut base) = read_mod(BASE_MOD, base_mod_folder()) {
        base.shipped = true;
        mods.push(base);
    }
    mods.sort_by(|a, b| a.id.cmp(&b.id));
    mods
}

/// Why no world can start with the mods of `plan`, made from `find_game_mods`: the base mod,
/// which holds every block, is off, cannot load or is missing.
pub fn base_mod_problem(mods: &[FoundMod], plan: &LoadPlan) -> Option<String> {
    let Some(index) = mods
        .iter()
        .position(|found| found.shipped && found.id == BASE_MOD)
    else {
        return Some(format!("its base mod, {BASE_MOD}, is missing"));
    };
    let name = mods[index].name();
    match &plan.states[index] {
        ModState::On => None,
        ModState::Off => Some(format!(
            "its base mod, {name}, is off. Turn it on in the Mods screen"
        )),
        ModState::Broken(problem) => Some(format!("its base mod, {name}, cannot load: {problem}")),
        ModState::Waiting(needs) => {
            let needs: Vec<String> = needs.iter().map(ToString::to_string).collect();
            Some(format!("its base mod, {name}, needs {}", needs.join("; ")))
        }
    }
}

/// Finds the mods of `folder`, by id: every folder with a `mod.toml` or an `init.luau`. A
/// folder that cannot be read has none.
pub fn find_mods(folder: &Path) -> Vec<FoundMod> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut mods: Vec<FoundMod> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            let id = path.file_name()?.to_str()?.to_string();
            read_mod(&id, path)
        })
        .collect();
    mods.sort_by(|a, b| a.id.cmp(&b.id));
    mods
}

/// The mod of the folder `id`, or `None` when the folder holds neither a manifest nor an
/// `init.luau`.
fn read_mod(id: &str, folder: PathBuf) -> Option<FoundMod> {
    let manifest_path = folder.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return folder
            .join(DEFAULT_ENTRY)
            .is_file()
            .then(|| FoundMod::undeclared(id, folder, None));
    }
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("Cannot read {MANIFEST_FILE}: {err}"))
        .and_then(|text| parse_manifest(&text));
    Some(match manifest {
        Ok(manifest) => {
            let problem = check_manifest(id, &manifest).err();
            FoundMod {
                id: id.to_string(),
                folder,
                manifest,
                declared: true,
                shipped: false,
                problem,
            }
        }
        Err(problem) => FoundMod {
            declared: true,
            ..FoundMod::undeclared(id, folder, Some(problem))
        },
    })
}

/// Reads the text of a `mod.toml`. The error is one line, for the Mods screen.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    toml::from_str(text).map_err(|err| {
        let message = err.message().to_string();
        let line = err
            .span()
            .map(|span| text[..span.start.min(text.len())].lines().count().max(1));
        match line {
            Some(line) => format!("{MANIFEST_FILE}:{line}: {message}"),
            None => format!("{MANIFEST_FILE}: {message}"),
        }
    })
}

/// Whether `id` can name a mod: lowercase letters, digits and `_`, at most `MAX_ID_LENGTH`.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LENGTH
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Checks the rules of the manifest of the folder `id` that its format alone does not.
fn check_manifest(id: &str, manifest: &Manifest) -> Result<(), String> {
    let about = &manifest.about;
    if !is_valid_id(&about.id) {
        return Err(format!(
            "The id \"{}\" must be lowercase letters, digits and _, at most {MAX_ID_LENGTH}",
            about.id
        ));
    }
    if about.id != id {
        return Err(format!(
            "The id \"{}\" differs from the folder's name, {id}",
            about.id
        ));
    }
    let name_length = about.name.trim().chars().count();
    if name_length == 0 || name_length > MAX_NAME_LENGTH {
        return Err(format!(
            "The name must have 1 to {MAX_NAME_LENGTH} characters"
        ));
    }
    if manifest.game.api == 0 || manifest.game.api > API_VERSION {
        return Err(format!(
            "Written for version {} of the API, the game has version {API_VERSION}",
            manifest.game.api
        ));
    }
    for (key, path) in [("entry", &about.entry), ("icon", &about.icon)] {
        if let Some(path) = path {
            if !stays_inside(path) {
                return Err(format!(
                    "The {key} \"{path}\" must be a path in the mod's folder"
                ));
            }
        }
    }
    for dependency in manifest.dependencies.keys().chain(manifest.optional.keys()) {
        if dependency == id {
            return Err("The mod depends on itself".into());
        }
    }
    Ok(())
}

/// Whether `path` is relative and never climbs out of the folder it starts in.
fn stays_inside(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// What `mods.ron` keeps: the ids of the mods turned off. Any other mod of the folder is on,
/// so a mod copied into it is on at the next start.
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct ModsSettings {
    pub disabled: BTreeSet<String>,
}

impl ModsSettings {
    pub fn is_on(&self, id: &str) -> bool {
        !self.disabled.contains(id)
    }

    pub fn set_on(&mut self, id: &str, on: bool) {
        if on {
            self.disabled.remove(id);
        } else {
            self.disabled.insert(id.to_string());
        }
    }
}

/// The settings of `mods.ron` in `game_folder`: none turned off when the file is missing. A
/// file that cannot be read turns none off either, and says why.
pub fn read_mods_settings(game_folder: &Path) -> Result<ModsSettings, String> {
    let path = game_folder.join(MODS_SETTINGS_PATH);
    match fs::read_to_string(&path) {
        Ok(text) => ron::de::from_str(&text)
            .map_err(|err| format!("Cannot read {MODS_SETTINGS_PATH}: {err}")),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ModsSettings::default()),
        Err(err) => Err(format!("Cannot read {MODS_SETTINGS_PATH}: {err}")),
    }
}

/// Writes `settings` to `mods.ron` in `game_folder`.
pub fn write_mods_settings(game_folder: &Path, settings: &ModsSettings) -> io::Result<()> {
    let ron = ron::ser::to_string_pretty(settings, ron::ser::PrettyConfig::new())
        .map_err(io::Error::other)?;
    let text = format!(
        "// The mods turned off in the Mods screen. Any other mod of mods/ is on.\n{ron}\n"
    );
    fs::write(game_folder.join(MODS_SETTINGS_PATH), text)
}

/// Why a dependency keeps a mod from loading.
#[derive(Debug, Clone, PartialEq)]
pub enum Unmet {
    /// No mod of the folder has this id.
    Absent,
    Off,
    /// It cannot load itself.
    Unloadable,
    /// Its version is not one the mod accepts.
    Version {
        wanted: VersionReq,
        found: Version,
    },
}

/// A dependency that keeps a mod from loading.
#[derive(Debug, Clone, PartialEq)]
pub struct Need {
    pub id: String,
    pub unmet: Unmet,
}

impl fmt::Display for Need {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = &self.id;
        match &self.unmet {
            Unmet::Absent => write!(f, "{id}, which is missing"),
            Unmet::Off => write!(f, "{id}, which is off"),
            Unmet::Unloadable => write!(f, "{id}, which cannot load"),
            Unmet::Version { wanted, found } => {
                write!(f, "{id} {wanted}, but {found} is there")
            }
        }
    }
}

/// What becomes of a mod at the next start.
#[derive(Debug, Clone, PartialEq)]
pub enum ModState {
    /// It loads, in its place in the order.
    On,
    /// The settings turn it off.
    Off,
    /// It cannot load: its manifest, or a cycle of dependencies.
    Broken(String),
    /// It waits for dependencies.
    Waiting(Vec<Need>),
}

/// What the next start does with the mods found: the state of each, and the order of those
/// that load.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadPlan {
    /// By index of the mods found.
    pub states: Vec<ModState>,
    /// The indices of the mods that load, dependencies first, else by id.
    pub order: Vec<usize>,
}

/// Plans the loading of `mods`, found by `find_mods`, with `settings`.
pub fn plan_loading(mods: &[FoundMod], settings: &ModsSettings) -> LoadPlan {
    let index_of: BTreeMap<&str, usize> = mods
        .iter()
        .enumerate()
        .map(|(index, found)| (found.id.as_str(), index))
        .collect();
    let mut states: Vec<ModState> = mods
        .iter()
        .map(|found| match &found.problem {
            Some(problem) => ModState::Broken(problem.clone()),
            None if !settings.is_on(&found.id) => ModState::Off,
            None => ModState::On,
        })
        .collect();

    // a mod waits for its unmet dependencies, until no other starts waiting
    loop {
        let mut changed = false;
        for index in 0..mods.len() {
            if states[index] != ModState::On {
                continue;
            }
            let needs: Vec<Need> = mods[index]
                .manifest
                .dependencies
                .iter()
                .filter_map(|(id, wanted)| {
                    let unmet = match index_of.get(id.as_str()) {
                        None => Unmet::Absent,
                        Some(&other) => match &states[other] {
                            ModState::Off => Unmet::Off,
                            ModState::Broken(_) | ModState::Waiting(_) => Unmet::Unloadable,
                            ModState::On if !wanted.matches(mods[other].version()) => {
                                Unmet::Version {
                                    wanted: wanted.clone(),
                                    found: mods[other].version().clone(),
                                }
                            }
                            ModState::On => return None,
                        },
                    };
                    Some(Need {
                        id: id.clone(),
                        unmet,
                    })
                })
                .collect();
            if !needs.is_empty() {
                states[index] = ModState::Waiting(needs);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // dependencies first: of the mods whose dependencies are all loaded, the first by id
    let loads = |index: usize, states: &[ModState]| states[index] == ModState::On;
    let before = |index: usize| -> Vec<usize> {
        let manifest = &mods[index].manifest;
        manifest
            .dependencies
            .keys()
            .chain(manifest.optional.keys())
            .filter_map(|id| index_of.get(id.as_str()).copied())
            .filter(|&other| loads(other, &states))
            .collect()
    };
    let befores: Vec<Vec<usize>> = (0..mods.len()).map(before).collect();
    let mut order = Vec::new();
    let mut placed = vec![false; mods.len()];
    loop {
        let next = (0..mods.len()).find(|&index| {
            loads(index, &states) && !placed[index] && befores[index].iter().all(|&o| placed[o])
        });
        let Some(next) = next else { break };
        placed[next] = true;
        order.push(next);
    }
    // what is left waits on itself, through a cycle
    let cycle: Vec<usize> = (0..mods.len())
        .filter(|&index| loads(index, &states) && !placed[index])
        .collect();
    if !cycle.is_empty() {
        let ids: Vec<&str> = cycle.iter().map(|&index| mods[index].id.as_str()).collect();
        let message = format!("Its dependencies make a cycle: {}", ids.join(", "));
        for index in cycle {
            states[index] = ModState::Broken(message.clone());
        }
    }
    LoadPlan { states, order }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANTERNS: &str = r#"
        [mod]
        id = "lanterns"
        name = "Lanterns"
        version = "1.2.0"
        description = "Adds the lantern."
        authors = ["MizKyosia"]

        [game]
        version = ">=0.4"
        api = 1

        [optional]
        wildflowers = ">=1.0"

        [[blocks]]
        id = "lantern"
        light = 15
    "#;

    fn write(folder: &Path, path: &str, text: &str) {
        let path = folder.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn manifest(id: &str, version: &str, dependencies: &[(&str, &str)]) -> String {
        let mut text = format!(
            "[mod]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\n[game]\napi = 1\n[dependencies]\n"
        );
        for (dependency, wanted) in dependencies {
            text += &format!("{dependency} = \"{wanted}\"\n");
        }
        text
    }

    fn mods_of(list: &[(&str, &str, &[(&str, &str)])]) -> (tempfile::TempDir, Vec<FoundMod>) {
        let folder = tempfile::tempdir().unwrap();
        for (id, version, dependencies) in list {
            write(
                folder.path(),
                &format!("{id}/{MANIFEST_FILE}"),
                &manifest(id, version, dependencies),
            );
        }
        let mods = find_mods(folder.path());
        (folder, mods)
    }

    fn order_of(mods: &[FoundMod], plan: &LoadPlan) -> Vec<String> {
        plan.order
            .iter()
            .map(|&index| mods[index].id.clone())
            .collect()
    }

    #[test]
    fn a_manifest_reads_and_leaves_out_the_keys_it_does_not_know() {
        let manifest = parse_manifest(LANTERNS).unwrap();
        assert_eq!(manifest.about.id, "lanterns");
        assert_eq!(manifest.about.version, Version::new(1, 2, 0));
        assert_eq!(manifest.about.authors, ["MizKyosia"]);
        assert_eq!(manifest.game.api, 1);
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.optional.contains_key("wildflowers"));
        assert!(check_manifest("lanterns", &manifest).is_ok());
    }

    #[test]
    fn a_broken_manifest_says_where() {
        let error = parse_manifest("[mod]\nid = \"lanterns\"\nname = 3\n").unwrap_err();
        assert!(error.starts_with("mod.toml:3:"), "{error}");
        let error =
            parse_manifest("[mod]\nid = \"x\"\nname = \"X\"\nversion = \"1\"\n").unwrap_err();
        assert!(error.contains("version"), "{error}");
    }

    #[test]
    fn a_manifest_breaking_a_rule_has_a_problem() {
        let problem = |id: &str, text: &str| check_manifest(id, &parse_manifest(text).unwrap());
        let good = manifest("tower", "1.0.0", &[]);
        assert!(problem("tower", &good).is_ok());
        assert!(problem("castle", &good).unwrap_err().contains("folder"));
        let upper = good.replace("id = \"tower\"", "id = \"Tower\"");
        assert!(problem("Tower", &upper).unwrap_err().contains("lowercase"));
        let later = good.replace("api = 1", "api = 2");
        assert!(problem("tower", &later).unwrap_err().contains("API"));
        let escaping = good.replace("[game]", "entry = \"../evil.luau\"\n[game]");
        assert!(problem("tower", &escaping).unwrap_err().contains("entry"));
        let selfish = manifest("tower", "1.0.0", &[("tower", "*")]);
        assert!(problem("tower", &selfish).unwrap_err().contains("itself"));
    }

    #[test]
    fn the_folder_holds_mods_with_or_without_a_manifest() {
        let folder = tempfile::tempdir().unwrap();
        write(folder.path(), "tower/init.luau", "-- old style");
        write(folder.path(), "lanterns/mod.toml", LANTERNS);
        write(folder.path(), "lanterns/icon.png", "png");
        write(folder.path(), "broken/mod.toml", "this is not toml");
        write(folder.path(), "notes/readme.txt", "not a mod");
        write(folder.path(), "loose.luau", "not in a folder");

        let mods = find_mods(folder.path());

        let ids: Vec<&str> = mods.iter().map(|found| found.id.as_str()).collect();
        assert_eq!(ids, ["broken", "lanterns", "tower"]);
        assert!(mods[0].problem.is_some());
        assert_eq!(mods[0].name(), "broken");
        assert_eq!(mods[1].name(), "Lanterns");
        assert_eq!(mods[1].entry(), None);
        assert_eq!(
            mods[1].icon(),
            Some(folder.path().join("lanterns/icon.png"))
        );
        assert!(!mods[2].declared);
        assert_eq!(mods[2].entry(), Some(folder.path().join("tower/init.luau")));
        assert_eq!(mods[2].icon(), None);
    }

    #[test]
    fn a_missing_folder_has_no_mods() {
        assert!(find_mods(Path::new("/no/such/folder")).is_empty());
    }

    #[test]
    fn dependencies_load_first_and_the_rest_by_id() {
        let (_folder, mods) = mods_of(&[
            ("alpha", "1.0.0", &[("zeta", "^1")]),
            ("beta", "1.0.0", &[]),
            ("zeta", "1.4.0", &[]),
        ]);
        let plan = plan_loading(&mods, &ModsSettings::default());
        assert_eq!(order_of(&mods, &plan), ["beta", "zeta", "alpha"]);
        assert!(plan.states.iter().all(|state| *state == ModState::On));
    }

    #[test]
    fn a_mod_waits_for_a_dependency_missing_off_or_of_another_version() {
        let (_folder, mods) = mods_of(&[
            ("base", "2.0.0", &[]),
            ("needs_base_1", "1.0.0", &[("base", "^1")]),
            ("needs_ghost", "1.0.0", &[("ghost", "*")]),
            ("needs_off", "1.0.0", &[("off", "*")]),
            ("off", "1.0.0", &[]),
            ("chain", "1.0.0", &[("needs_off", "*")]),
        ]);
        let mut settings = ModsSettings::default();
        settings.set_on("off", false);
        let plan = plan_loading(&mods, &settings);
        let state = |id: &str| &plan.states[mods.iter().position(|m| m.id == id).unwrap()];
        let unmet = |id: &str| match state(id) {
            ModState::Waiting(needs) => needs[0].unmet.clone(),
            other => panic!("{id} is {other:?}"),
        };

        assert_eq!(order_of(&mods, &plan), ["base"]);
        assert!(matches!(unmet("needs_base_1"), Unmet::Version { .. }));
        assert_eq!(unmet("needs_ghost"), Unmet::Absent);
        assert_eq!(unmet("needs_off"), Unmet::Off);
        assert_eq!(*state("off"), ModState::Off);
        assert_eq!(unmet("chain"), Unmet::Unloadable);
    }

    #[test]
    fn an_optional_dependency_loads_first_when_there_and_is_not_needed() {
        let (folder, _) = mods_of(&[("aaa", "1.0.0", &[]), ("zzz", "1.0.0", &[])]);
        write(
            folder.path(),
            "aaa/mod.toml",
            &(manifest("aaa", "1.0.0", &[]) + "[optional]\nzzz = \"*\"\nnowhere = \"*\"\n"),
        );
        let mods = find_mods(folder.path());
        let plan = plan_loading(&mods, &ModsSettings::default());
        assert_eq!(order_of(&mods, &plan), ["zzz", "aaa"]);

        let mut settings = ModsSettings::default();
        settings.set_on("zzz", false);
        let plan = plan_loading(&mods, &settings);
        assert_eq!(order_of(&mods, &plan), ["aaa"]);
    }

    #[test]
    fn a_cycle_of_dependencies_breaks_every_mod_in_it() {
        let (_folder, mods) = mods_of(&[
            ("free", "1.0.0", &[]),
            ("hen", "1.0.0", &[("egg", "*")]),
            ("egg", "1.0.0", &[("hen", "*")]),
        ]);
        let plan = plan_loading(&mods, &ModsSettings::default());
        assert_eq!(order_of(&mods, &plan), ["free"]);
        let broken = plan
            .states
            .iter()
            .filter(
                |state| matches!(state, ModState::Broken(message) if message.contains("egg, hen")),
            )
            .count();
        assert_eq!(broken, 2);
    }

    #[test]
    fn the_settings_are_written_and_read_back() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(
            read_mods_settings(folder.path()),
            Ok(ModsSettings::default())
        );
        let mut settings = ModsSettings::default();
        settings.set_on("gold_rush", false);
        settings.set_on("fox_dens", false);
        settings.set_on("fox_dens", true);
        write_mods_settings(folder.path(), &settings).unwrap();

        assert_eq!(read_mods_settings(folder.path()), Ok(settings.clone()));
        let text = fs::read_to_string(folder.path().join(MODS_SETTINGS_PATH)).unwrap();
        assert!(text.starts_with("// The mods turned off"), "{text}");
        assert!(
            text.contains("gold_rush") && !text.contains("fox_dens"),
            "{text}"
        );

        fs::write(folder.path().join(MODS_SETTINGS_PATH), "(disabled: [").unwrap();
        assert!(read_mods_settings(folder.path()).is_err());
    }

    #[test]
    fn a_game_folder_runs_the_base_mod_with_its_own() {
        let game = tempfile::tempdir().unwrap();
        let mods = find_game_mods(game.path());
        assert_eq!(mods.len(), 1);
        assert!(
            mods[0].shipped && mods[0].problem.is_none(),
            "{:?}",
            mods[0]
        );
        assert_eq!(mods[0].id, BASE_MOD);
        assert!(mods[0].icon().is_some());

        write(
            &game.path().join(MODS_FOLDER),
            &format!("castle/{MANIFEST_FILE}"),
            &manifest("castle", "1.0.0", &[(BASE_MOD, "^0.1")]),
        );
        let mods = find_game_mods(game.path());
        let ids: Vec<&str> = mods.iter().map(|found| found.id.as_str()).collect();
        assert_eq!(ids, ["castle", BASE_MOD]);
        assert_eq!(plan_loading(&mods, &ModsSettings::default()).order, [1, 0]);
    }

    #[test]
    fn a_mod_of_the_folder_cannot_take_the_id_of_the_base_mod() {
        let game = tempfile::tempdir().unwrap();
        write(
            &game.path().join(MODS_FOLDER),
            &format!("{BASE_MOD}/{MANIFEST_FILE}"),
            &manifest(BASE_MOD, "9.0.0", &[]),
        );
        let mods = find_game_mods(game.path());
        assert_eq!(mods.len(), 2);
        let impostor = mods.iter().find(|found| !found.shipped).unwrap();
        assert!(impostor
            .problem
            .as_ref()
            .unwrap()
            .contains("game's own mod"));
        let plan = plan_loading(&mods, &ModsSettings::default());
        assert_eq!(base_mod_problem(&mods, &plan), None);
    }

    #[test]
    fn no_world_starts_while_the_base_mod_is_off_nor_do_the_mods_that_need_it() {
        let game = tempfile::tempdir().unwrap();
        write(
            &game.path().join(MODS_FOLDER),
            &format!("castle/{MANIFEST_FILE}"),
            &manifest("castle", "1.0.0", &[(BASE_MOD, "^0.1")]),
        );
        let mods = find_game_mods(game.path());
        let mut settings = ModsSettings::default();
        assert_eq!(
            base_mod_problem(&mods, &plan_loading(&mods, &settings)),
            None
        );

        settings.set_on(BASE_MOD, false);
        let plan = plan_loading(&mods, &settings);
        let problem = base_mod_problem(&mods, &plan).unwrap();
        assert_eq!(
            problem,
            "its base mod, Rustcraft, is off. Turn it on in the Mods screen"
        );
        assert_eq!(
            plan.states[0],
            ModState::Waiting(vec![Need {
                id: BASE_MOD.into(),
                unmet: Unmet::Off
            }])
        );
        assert!(base_mod_problem(&[], &plan_loading(&[], &settings))
            .unwrap()
            .contains("missing"));
    }
}
