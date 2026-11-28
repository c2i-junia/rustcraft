//! The stats of the mobs, how they are drawn and heard, as the content files define them, one
//! entry by kind (`docs/design/content-registry.md`, 2.4):
//!
//! ```ron
//! (
//!     mobs: {
//!         "sheep": (
//!             health: 8, size: (0.9, 1.25, 0.9), walk_speed: 1.0,
//!             eats: ["rye"],
//!             drops: [(item: "raw_mutton", count: Range(1, 2)), (item: "wool", when: NotSheared)],
//!             model: "models/animated/Sheep.glb", clips: [Eat, Idle, Walk], sounds: "sheep",
//!         ),
//!     },
//! )
//! ```
//!
//! `MobKind` stays an enum: a file gives each of its kinds its numbers, not its behaviour, and
//! must give every kind.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::files::{present, Kind, Named, Names};
use super::{ContentError, ItemProps};
use crate::utils::snake_case;
use crate::world::hostiles::{Hunt, Strike};
use crate::world::{ItemId, Loot, LootCount, LootWhen, MobKind};

/// What a mob looks like it does: a clip of the animations of its model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobClip {
    Idle,
    Walk,
    Run,
    Eat,
    /// A strike, played once.
    Attack,
}

impl MobClip {
    /// Whether the clip plays once rather than over and over.
    pub fn plays_once(self) -> bool {
        self == MobClip::Attack
    }
}

/// The numbers of a kind of mob, as its getters read them: see each getter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MobProps {
    /// `MobKind::health`.
    pub health: u32,
    /// `MobKind::size`: width, height and depth.
    pub size: [f32; 3],
    /// `MobKind::walk_speed`.
    pub walk_speed: f32,
    /// `MobKind::hunt`.
    pub hunt: Option<Hunt>,
    /// `MobKind::strike`.
    pub strike: Option<Strike>,
    /// `MobKind::experience`.
    pub experience: u32,
    /// `MobKind::is_tameable`.
    pub tameable: bool,
    /// `MobKind::eats`.
    pub eats: Vec<ItemId>,
    /// `MobKind::loot`.
    pub drops: Vec<Loot>,
    /// The model it is drawn with, in the assets folder: `models/animated/Sheep.glb`.
    pub model: String,
    /// The clips of its model, in the order of its animations, which the exporter sorts by
    /// name: the index of each is that of its animation.
    pub clips: Vec<MobClip>,
    /// How far above the origin of its model the bottom of its box is, in metres: the model is
    /// drawn that much lower than its box.
    pub lift: f32,
    /// The folder of its sounds in `sounds/mob/`.
    pub sounds: String,
}

/// A kind of mob in a file. What it leaves out is none: no hunt, no strike, no experience, not
/// tameable, eats nothing, drops nothing, no lift.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct MobDef {
    pub health: u32,
    pub size: [f32; 3],
    pub walk_speed: f32,
    #[serde(default, deserialize_with = "present")]
    pub hunt: Option<Hunt>,
    #[serde(default, deserialize_with = "present")]
    pub strike: Option<Strike>,
    #[serde(default)]
    pub experience: u32,
    #[serde(default)]
    pub tameable: bool,
    #[serde(default)]
    pub eats: Vec<String>,
    #[serde(default)]
    pub drops: Vec<LootDef>,
    pub model: String,
    pub clips: Vec<MobClip>,
    #[serde(default)]
    pub lift: f32,
    pub sounds: String,
}

impl Kind for MobDef {
    const KIND: &'static str = "mob";
}

/// A line of loot in a file: one `item` by default, always.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct LootDef {
    pub item: String,
    #[serde(default = "one")]
    pub count: LootCount,
    #[serde(default)]
    pub when: LootWhen,
}

fn one() -> LootCount {
    LootCount::Range(1, 1)
}

/// The name of `kind` in the files: `gust_owl` for `MobKind::GustOwl`.
pub fn mob_name(kind: MobKind) -> String {
    snake_case(&format!("{kind:?}"))
}

/// The props of every kind of `MobKind::ALL`, in its order, from the `mobs` of `files`, the
/// definitions of each file by its path, against the items of `names`, whose properties are
/// `items`. Every kind must be defined, in one file.
pub(super) fn build(
    files: Vec<(&Path, Named<MobDef>)>,
    folder: &Path,
    names: &Names,
    items: &[ItemProps],
) -> Result<Vec<MobProps>, ContentError> {
    let fail = |file: &Path, reason: String| ContentError {
        file: file.to_path_buf(),
        reason,
    };
    let kind_names: Vec<String> = MobKind::ALL.into_iter().map(mob_name).collect();
    let mut mobs: Vec<Option<(MobProps, &Path)>> = vec![None; MobKind::ALL.len()];
    for (file, defs) in files {
        for (name, def) in defs.0 {
            let in_mob = |reason: String| fail(file, format!("mob \"{name}\": {reason}"));
            let index = kind_names
                .iter()
                .position(|known| *known == name)
                .ok_or_else(|| in_mob("no kind of mob is named so".to_string()))?;
            let props = build_mob(def, names, items).map_err(in_mob)?;
            if let Some((_, other)) = mobs[index].replace((props, file)) {
                return Err(fail(
                    file,
                    format!("mob \"{name}\" is also defined in {}", other.display()),
                ));
            }
        }
    }
    mobs.into_iter()
        .zip(&kind_names)
        .map(|(mob, name)| {
            mob.map(|(props, _)| props)
                .ok_or_else(|| fail(folder, format!("mob \"{name}\" is defined in no file")))
        })
        .collect()
}

/// The item `name` names, which a file must define (not a placeholder).
fn defined_item(
    names: &Names,
    items: &[ItemProps],
    field: &str,
    name: &str,
) -> Result<ItemId, String> {
    let item = names.item(field, name)?;
    if items[item.index()].placeholder {
        return Err(format!(
            "{field}: the item \"{name}\" is defined in no file"
        ));
    }
    Ok(item)
}

fn build_mob(def: MobDef, names: &Names, items: &[ItemProps]) -> Result<MobProps, String> {
    if def.health == 0 {
        return Err("health 0: a mob has 1 point at least".to_string());
    }
    if def.size.iter().any(|side| side.is_nan() || *side <= 0.0) {
        return Err(format!("size {:?}: each side is above 0", def.size));
    }
    if !def.model.ends_with(".glb") {
        return Err(format!("model \"{}\": a .glb file", def.model));
    }
    if let Some(i) = (1..def.clips.len()).find(|&i| def.clips[..i].contains(&def.clips[i])) {
        return Err(format!("clips: {:?} is listed twice", def.clips[i]));
    }
    // what `clip_for` falls back to
    for needed in [MobClip::Idle, MobClip::Walk] {
        if !def.clips.contains(&needed) {
            return Err(format!("clips: a model has {needed:?}"));
        }
    }
    if !super::files::is_content_name(&def.sounds) {
        return Err(format!(
            "sounds \"{}\": a folder is words in lowercase joined by _",
            def.sounds
        ));
    }
    let eats = def
        .eats
        .iter()
        .map(|name| defined_item(names, items, "eats", name))
        .collect::<Result<Vec<_>, _>>()?;
    let drops = def
        .drops
        .iter()
        .map(|line| {
            let item = defined_item(names, items, "drops", &line.item)?;
            check_count(line.count)
                .map_err(|reason| format!("drops: \"{}\": {reason}", line.item))?;
            Ok(Loot {
                item,
                count: line.count,
                when: line.when,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(MobProps {
        health: def.health,
        size: def.size,
        walk_speed: def.walk_speed,
        hunt: def.hunt,
        strike: def.strike,
        experience: def.experience,
        tameable: def.tameable,
        eats,
        drops,
        model: def.model,
        clips: def.clips,
        lift: def.lift,
        sounds: def.sounds,
    })
}

/// Why `count` cannot be rolled (`roll_loot`), which would stop the server.
fn check_count(count: LootCount) -> Result<(), String> {
    match count {
        LootCount::Range(min, max) if min > max => Err(format!("Range({min}, {max}): min > max")),
        LootCount::Odds(n, d) if d == 0 || n > d => {
            Err(format!("Odds({n}, {d}): n in d, d above 0 and n at most d"))
        }
        LootCount::Chance(p) if !(0.0..=1.0).contains(&p) => {
            Err(format!("Chance({p}): from 0 to 1"))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::files::tests::game_folder;
    use crate::content::Content;
    use bevy::math::Vec3;

    #[test]
    fn the_mobs_of_the_files_are_those_of_the_code() {
        let content = Content::load(&game_folder()).unwrap();
        assert_eq!(content.mobs.len(), MobKind::ALL.len());
        for (index, kind) in MobKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(), index);
            let mob = content.mob(kind);
            assert_eq!(mob.health, kind.health(), "{kind:?}");
            assert_eq!(Vec3::from(mob.size), kind.size(), "{kind:?}");
            assert_eq!(mob.walk_speed, kind.walk_speed(), "{kind:?}");
            assert_eq!(mob.hunt, kind.hunt(), "{kind:?}");
            assert_eq!(mob.strike, kind.strike(), "{kind:?}");
            assert_eq!(mob.experience, kind.experience(), "{kind:?}");
            assert_eq!(mob.tameable, kind.is_tameable(), "{kind:?}");
            for index in 0..content.items.len() {
                let item = ItemId::from_index(index as u16);
                assert_eq!(
                    mob.eats.contains(&item),
                    kind.eats(item),
                    "{kind:?} {item:?}"
                );
            }
            assert_eq!(mob.drops, kind.loot(), "{kind:?}");
            assert_eq!(mob.sounds, mob_name(kind), "{kind:?}");
        }
    }

    const FOX: &str = r#""fox": (
            health: 10,
            size: (0.7, 0.7, 0.7),
            walk_speed: 1.6,
            eats: ["berries"],
            model: "models/animated/Fox.glb",
            clips: [Idle, Run, Walk],
            sounds: "fox",
        ),"#;

    /// A copy of the content files of the game, in which `from` is replaced by `to` in
    /// `mobs.ron`, where it is written once.
    fn mobs_edited(from: &str, to: &str) -> tempfile::TempDir {
        let folder = super::super::files::tests::edited_game_folder(&[]);
        let path = folder.path().join("mobs.ron");
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(from).count(), 1, "{from}");
        std::fs::write(&path, text.replace(from, to)).unwrap();
        folder
    }

    #[test]
    fn an_edited_mob_reads_its_new_numbers() {
        let folder = mobs_edited(
            "health: 10,\n            size: (0.7, 0.7, 0.7),",
            "health: 11,\n            size: (0.7, 0.7, 0.7),\n            lift: 0.5,",
        );
        let content = Content::load(folder.path()).unwrap();
        assert_eq!(content.mob(MobKind::Fox).health, 11);
        assert_eq!(content.mob(MobKind::Fox).lift, 0.5);
        let game = Content::load(&game_folder()).unwrap();
        assert_eq!(content.mob(MobKind::Wolf), game.mob(MobKind::Wolf));
        assert!(!content.same_properties(&game));
    }

    #[test]
    fn a_bad_mob_is_refused_with_its_file_and_its_entry() {
        let fox = |from: &str, to: &str| FOX.replace(from, to);
        let drops = |line: &str| fox("eats", &format!("drops: [{line}],\n            eats"));
        let cases = [
            (String::new(), "mob \"fox\" is defined in no file"),
            (
                format!("{FOX}{}", fox("\"fox\"", "\"dragon\"")),
                "mob \"dragon\": no kind of mob is named so",
            ),
            (
                fox("berries", "bery"),
                "mob \"fox\": eats: no item is named \"bery\"",
            ),
            (
                drops("(item: \"stik\")"),
                "mob \"fox\": drops: no item is named \"stik\"",
            ),
            (
                drops("(item: \"stick\", count: Odds(1, 0))"),
                "mob \"fox\": drops: \"stick\": Odds(1, 0)",
            ),
            (
                drops("(item: \"stick\", count: Range(3, 2))"),
                "mob \"fox\": drops: \"stick\": Range(3, 2)",
            ),
            (
                drops("(item: \"stick\", count: Chance(1.5))"),
                "mob \"fox\": drops: \"stick\": Chance(1.5)",
            ),
            (
                fox("[Idle, Run, Walk]", "[Idle, Jump, Walk]"),
                "mob \"fox\"",
            ),
            (
                fox("[Idle, Run, Walk]", "[Idle, Run]"),
                "mob \"fox\": clips: a model has Walk",
            ),
            (
                fox("[Idle, Run, Walk]", "[Idle, Walk, Idle]"),
                "mob \"fox\": clips: Idle is listed twice",
            ),
            (
                fox("Fox.glb", "Fox.gltf"),
                "mob \"fox\": model \"models/animated/Fox.gltf\": a .glb file",
            ),
            (fox("health: 10", "health: 0"), "mob \"fox\": health 0"),
            (
                fox("(0.7, 0.7, 0.7)", "(0.7, 0.0, 0.7)"),
                "mob \"fox\": size [0.7, 0.0, 0.7]",
            ),
            (
                fox("sounds: \"fox\"", "sounds: \"Fox\""),
                "mob \"fox\": sounds \"Fox\"",
            ),
        ];
        for (to, expected) in cases {
            let folder = mobs_edited(FOX, &to);
            let err = Content::load(folder.path()).unwrap_err();
            assert!(
                err.file.ends_with("mobs.ron") || err.file == folder.path(),
                "{err}"
            );
            assert!(err.reason.contains(expected), "{expected}: {err}");
        }
        // a kind defined in two files names both
        let folder = super::super::files::tests::game_folder_with(&format!("(mobs: {{{FOX}}})"));
        let err = Content::load(folder.path()).unwrap_err();
        assert!(err.file.ends_with("mobs.ron"), "{err}");
        assert!(
            err.reason.contains("mob \"fox\" is also defined in"),
            "{err}"
        );
        assert!(err.reason.contains("extra.ron"), "{err}");
    }
}
