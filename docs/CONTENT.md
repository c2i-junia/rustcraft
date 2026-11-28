# Content files

The blocks, items, recipes, smelting, fuel, tags, families and mobs of the base game are
data, in `data/content/rustcraft/*.ron`. Adding a block, a recipe or a wood is an edit to those
files plus its textures: no Rust, no protocol bump, no rebuild.

```
just check-content        # validates the files in about a second, no game, no world
/reload                   # in a running game or server: reads them again now
```

The fields below are those of the structs of `shared/src/content/` (`BlockDef` and `ItemDef` in
`files.rs`, `RecipeDef` and `FuelDef` in `recipes.rs`, `FamilyDef` in `families.rs`, `MobDef` in
`mobs.rs`). Every example of this page that is fenced as `ron` is loaded by a test
(`docs_examples_validate` in `shared/src/content/mod.rs`), each added to a copy of the game's
files. The one fenced `ron schema-only` (a mob, which a file cannot add) is parsed only.

## Where the files are

Every `*.ron` of `data/content/rustcraft/` is read, in the order of its name, and the names are
what matters, not the file: a block or an item is defined in one file only (two files that
define it are an error naming both). A new file, `amber.ron`, is enough for a new feature.
Each file is one `( ... )` with any of these keys, all optional: `blocks`, `items`, `tags`,
`recipes`, `smelting`, `fuel`, `families`, `mobs`. `//` comments are fine. The names are
snake_case, as the commands name them (`oak_log`); the texture of a block or an item is named
in PascalCase (`OakLog`), in `data/graphics/base_textures/blocks/` and `.../items/`.

Unknown fields are errors (`lite: 9` fails and names `lite`), so a typo cannot pass silently.

## Blocks

```ron
(
    blocks: {
        "amber": (
            name: (en: "Amber", fr: "Ambre"),
            look: Transparent,
            break: 8,
            tools: [Pickaxe],
            light: 6,
            faces: (all: "Amber"),
        ),
    },
)
```

That is a whole block, its item (see below) and its drops: breaking it gives the item `amber`.
A field left out takes its default:

| Field | Meaning | Default |
|---|---|---|
| `name: (en: .., fr: ..)` | Display name, per language; one language may be left out | the language files (`data/lang/`), else the name |
| `like: "block"` | Takes every field it leaves out from that block (itself completed first), but `name` and `aliases` | none |
| `aliases: [..]` | Former names, see Renaming | none |
| `look` | `Solid`, `Transparent`, `Liquid`, `Decoration` | `Solid` |
| `shape` | `Cube`, `Flora`, `Plate`, `Torch`, `WallTorch`, `Pod`, `Box`, `Fence`, `Sign`, `Campfire`: each is drawn by its own code, a new one is a Rust change | `Cube` |
| `faces: (all, sides, top, bottom, front, back, left, right)` | Texture of each face, `all` then `sides` then the face | the texture of the block's name on every face |
| `overlay: "Tex"` | Texture drawn over the sides (grass) | none |
| `tint` | `None`, `Grass`, `Foliage`, `Green`, `Water` | `None` |
| `sway` | `None`, `Whole`, `Top` (wind) | `None` |
| `break: n` | Hits a hand breaks it in; without it, it cannot be broken | unbreakable |
| `tools: [Pickaxe, Axe, Shovel, Hoe, Sword]` | Tools that mine it faster | none |
| `needs_tool: (kind: Pickaxe, level: 1)` | Tool and mining level that harvest it | none |
| `drops: [(chance, "item", count)]` | One entry is rolled, by relative `chance`; `drops: []` drops nothing | the item of the block's name |
| `light: 0..15` | Light it emits | 0 |
| `opacity: 0..15` | Light it takes | from `look` (decoration and glass 0, solid and other 1) |
| `collision`, `outline` | `FullBlock`, `None` or `Aabb((min: (x, y, z), max: (x, y, z)))`, inside the cell, up to 1.5 high | `FullBlock`; `outline` is `collision` |
| `usable`, `replaceable`, `tall`, `glass`, `container`, `submerged`, `hangs` | The flags: right click uses it, placing replaces it, fence-high, glass panes reach it, holds items, grows under water, hangs from above | false |
| `behaviour` | `None` or `Furnace(lit: false)` (a new behaviour is a Rust change) | `None` |
| `sapling`, `soil: [..]`, `toggled`, `top`, `hardens` | Block or item names for leaves, plants, doors and gates, concrete | none |
| `palette: (form, base, item)` | A slab, stairs, door, trapdoor or gate of a base block (`Form`) | none |

`like` is the way to make a variant:

```ron
(
    blocks: {
        "amber_bricks": (like: "stone", faces: (all: "AmberBricks"), drops: [(1, "cobblestone", 1)]),
        "amber_lamp": (like: "amber_bricks", light: 14, break: 4, faces: (all: "AmberLamp")),
    },
)
```

## Items

Every block has an item of its name, **implicit**: it places the block, stacks to 64, and is in
the category `Blocks`. Write an `items` entry only to change that or to make an item that is
no block:

```ron
(
    items: {
        "amber_shard": (stack: 16, category: Materials, name: (en: "Amber shard")),
        "amber_jam": (food_value: (4, 4.8), crafting_remains: "glass_vial", category: Materials),
        "amber_pickaxe": (
            stack: 1, attack_damage: 3, durability: 200, category: Tools,
            tool: (kind: Pickaxe, tier: Iron, speed: 3, durability: 200, wear_per_block: 1),
            default_type: Tool(wear: 0),
        ),
    },
)
```

Fields: `name`, `aliases`, `stack` (64), `attack_damage` (1), `food_value: (hunger,
saturation)`, `food_effect: (effect, level, ticks)`, `eaten_remains` and `crafting_remains` (an
item left behind), `horse_armour`, `spawned_mob` (a spawn egg: a `MobKind`), `tool`,
`durability`, `category` (`Blocks`, `Wood`, `Nature`, `Ores`, `Materials`, `Stations`,
`Tools`, `Mobs`), `places` (the block it places, by default the block of its name) and
`default_type` (`Generic`, or `Tool(wear: 0)` for an item that wears out). `places` and
`default_type` exclude each other. A tool needs `tool`, `durability`, `stack: 1` and the
`Tool` type, as in `tools.ron`.

## Recipes

Tried in the order of the files, then of each file. `count` is 1 by default.

```ron
(
    recipes: [
        // a pattern: a space is an empty cell, each other character is named in `key`
        (out: "amber_bricks", count: 4, pattern: ["AA", "AA"], key: {'A': "amber"}),
        // a tag, written #name, takes any item of the tag
        (out: "amber_lamp", pattern: ["AAA", "ATA", "AAA"], key: {'A': "amber", 'T': "torch"}),
        (out: "amber_chest", pattern: ["PPP", "PAP", "PPP"], key: {'P': "#planks", 'A': "amber"}),
        // shapeless: the items in any place of the grid
        (out: "amber", count: 2, shapeless: ["stone", "#wool"]),
    ],
    blocks: {
        "amber": (),
        "amber_bricks": (like: "stone"),
        "amber_lamp": (like: "amber", light: 14),
        "amber_chest": (like: "chest"),
    },
)
```

The pattern is at most 3x3; the rows may have different lengths. An item that no file defines
is an error naming the recipe.

## Smelting, fuel and tags

```ron
(
    tags: {
        "gems": ["diamond", "gold_ingot"],
    },
    smelting: [
        ("amber", "glass"),
        ("#gems", "stone"),
    ],
    fuel: {
        "amber": 400,
        "#gems": 800,
    },
    blocks: {
        "amber": (),
    },
)
```

`smelting` is a list of `(input, output)`; `fuel` maps an item to the ticks it burns in a
furnace. Both take a `#tag` for each item of the tag. A tag is a name for a list of items; a
file's tag adds to the tag of the same name in the other files and in the engine (`#planks`,
`#wool`), so a new wood joins `#planks` and every recipe that takes planks.

## Families

A family writes definitions once for each member of a list: the slabs and stairs of a wood, a
colour of wool. It is in `families`, and it holds the keys `blocks`, `items`, `tags`, `recipes`,
`smelting` and `fuel`, as a file does.

```ron
(
    families: [
        (
            each: {"g": ["amber", "opal"]},
            except: ["opal_slab"],
            blocks: {
                "{g}_block": (like: "stone", faces: (all: "{G}Block")),
                "{g}_slab": (
                    like: "{g}_block",
                    palette: (form: Slab, base: "{g}_block", item: "{g}_slab"),
                    look: Decoration,
                    collision: Aabb((min: (0, 0, 0), max: (1, 0.5, 1))),
                    faces: (all: "{G}Block"),
                ),
            },
            tags: {"gem_blocks": ["{g}_block"]},
            recipes: [
                (out: "{g}_slab", count: 6, pattern: ["BBB"], key: {'B': "{g}_block"}),
            ],
            fuel: {"{g}_slab": 150},
        ),
    ],
)
```

- `each` names the lists: `{g}` stands for each word (`amber`), `{G}` for its Rust name
  (`Amber`, `OakPlanks` for `oak_planks`). Several lists are read side by side, one word of
  each per member: `each: {"r": ["stone", "brick"], "m": ["stone", "brick_wall"]}`.
- The substitution is made in every name and every text of the definitions.
- `except` lists names, once substituted, that the family does not make.
- A block or an item that a file writes out under the name the family would give it (or one
  of its `aliases`) is the file's: the family's is dropped, and the check tells it
  ("replaced by the one ... writes out"). The same for a recipe with the same output and
  cells.
- The recipes of a family follow those of the file, member after member.

**A new wood** (`palette.ron`, the first family) is a word in its `each` list, and its
textures in `data/graphics/base_textures/blocks/`: `<Wood>Log`, `<Wood>LogTop`,
`<Wood>Planks`, `<Wood>Door`, `<Wood>DoorTop`, `<Wood>Trapdoor`, as `JungleLog` for the jungle.
The log, planks, slab, stairs, door, trapdoor and fence gate, their recipes, their fuel, the
`#planks` tag and the log's charcoal smelting come with the word. A new rock or dye colour is
a word in the list of its family the same way.

## Aliases and renaming

`aliases: ["old_name"]` on a block or an item makes the old name find it: in saved worlds, in
the commands and in other files. To rename a block, give it the new key and put the old one in
`aliases`. An alias must be a name that no other block (or item) has.

```ron
(
    blocks: {
        "amber_block": (like: "stone", aliases: ["amber_stone"], faces: (all: "AmberBlock")),
    },
)
```

## Mobs

`mobs.ron` has one entry per kind of `MobKind`, and only those: a new kind is a Rust change,
which gives it its behaviour. The file gives it its numbers, so an example cannot be a new
file (a kind is defined in one file only) and is parsed, not loaded:

```ron schema-only
(
    mobs: {
        "fox": (
            health: 10,                      // points; required, as the next five
            size: (0.7, 0.7, 0.7),           // width, height, depth of its box, in blocks
            walk_speed: 1.6,                 // blocks/s
            model: "models/animated/Fox.glb",
            clips: [Idle, Run, Walk],        // in the order of the model's animations; needs Idle and Walk
            sounds: "fox",                   // folder in sounds/mob/
            hunt: (sight: 16.0, forget: 24.0, speed: 2.8),                    // a hostile's
            strike: (damage: 2, reach: 1.0, cooldown: 15, knock_back: 0.0),   // a hostile's
            experience: 3,                   // 0
            tameable: false,                 // false; tamed by feeding it `eats`
            eats: ["berries"],
            drops: [(item: "stick", count: Range(1, 3), when: Always)],
            lift: 0.0,                       // metres its box is above its model's origin
        ),
    },
)
```

`drops` lines are `item`, `count` (`Range(min, max)`, `Odds(n, d)` or `Chance(p)`; default one)
and `when` (`Always`, `NotSheared`, `Saddled` or `Otherwise`, default `Always`). A spawn egg is
an item with `spawned_mob: Fox`.

## Checking, and how a reload works

`just check-content` (the server's `--check-content [DIR]`: no DIR means the folder the server
reads) loads the files exactly as the game does, writes the warnings to stdout and the first
error to stderr, and exits 0 only when they are valid. An error is
`<file>: block "amber": light 99 is above 15`: the file, the key, the reason. A syntax or
unknown-field error names the file and the key, with the position of a syntax error.

In a running server (solo included), the files are watched: a moment (0.2 s) after one is
saved, all of them are read again. `/reload` does it at once, for the files edited somewhere
the watcher misses. The result is all or nothing:

- **Valid**: the new content is installed, sent to every player (a client rebuilds its
  meshes, recipes and item browser), and told in the chat and the log. A block keeps its number.
- **An error**: nothing changes. The error goes to the server's log and to the chat of every
  player (`Content files not reloaded: <file>: <key>: <reason>`), and the game goes on with
  the content it had. At the start of a server, an error stops it instead.
- **Warnings**: a block or an item that no earlier content had is told, as `block "tourch" in
  amber.ron is new; did you mean "torch"?`. A misspelled key that defines a new block is not an
  error, because it is valid: the warning is how it is found. A misspelled name that is
  *referred* to (in a recipe, `like`, `drops`...) is an error.

**A removed pack**: when a file, a block or an item is deleted, a world that holds it still
loads. The blocks keep their number as placeholders (drawn as the debug block, dropping
nothing) and the items as inert placeholders, until the name is defined again, when they come
back. Nothing in a save is lost until the player breaks it.

**A renamed block**: use `aliases` (above); without it the old name is a removed block and the
new name a new one.
