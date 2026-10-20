# Jungle Lot 1 (Jungle wood) Implementation Plan

**Goal:** The wood of the jungle, with no jungle yet: `JungleLog`, `JunglePlanks` and
`JungleLeaves` as blocks and items, with their textures, break times, tools, drops, sounds,
render (the leaves under the new `JUNGLE_LEAVES_COLOR` tint), minimap colours and item icons,
and the nine recipes of jungle wood. `PROTOCOL_ID` goes from 19 to 20. To see: `/give
jungle_log`, place, break, craft.

**Spec:** `docs/superpowers/specs/2026-10-20-jungle-biome-design.md`, the rows "New blocks",
"New items", "Block behaviour", "Recipes", "Tint", "Textures" and "Protocol" of "Decisions",
the "Client" paragraph of "Architecture", and lot 1 of "Lots".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `BlockId` and `ItemId` are append only: the new variants go at the end, and the pinned
  encodings grow by as many lines.
- Jungle wood mirrors spruce wherever spruce is wired: `shared` (blocks, items, recipes),
  `client` (render, sounds), `server/examples/worldgen_dump.rs` and
  `scripts/worldgen_preview.py`.
- `cargo test`, `cargo clippy --all-targets -- -D warnings` on `shared`, `client` and `server`,
  and `cargo fmt --all -- --check` are clean at the commit.
- Nothing generates the new blocks yet: `server/src/world/generation.rs` (and its
  `precedence`) is left to lot 2.

## Decisions (where the spec is silent)

1. **Order of the variants:** `BlockId::{JungleLog, JunglePlanks, JungleLeaves}` are 28, 29
   and 30; `ItemId::{JungleLog, JunglePlanks, JungleLeaves}` are 35, 36 and 37, in the order
   the spec lists them.
2. **`JUNGLE_LEAVES_COLOR`** lives next to `GRASS_COLOR` in `client/src/constants.rs`, an
   `[f32; 4]` like it: `[0.05, 0.62, 0.05, 1.0]`, the spec's colour with an opaque alpha.
3. **Item icons.** An item with no texture of its own shows its block texture, untinted:
   today the oak leaves, tall grass and grass icons show the grey textures their blocks tint
   in the world. The spec wants the icons of the tinted jungle blocks to look as they do in
   the world, so `items/JungleLeaves.png` is the block texture tinted by
   `JUNGLE_LEAVES_COLOR` as the GPU tints it: each channel turned linear, multiplied, and
   turned back to sRGB, the alpha kept. A client test checks the icon against the block
   texture and the constant, so that they cannot drift apart. The log and the planks are not
   tinted and need no icon: they show their block texture, the log its bark, like the oak log.
   The oak leaves, tall grass and grass icons are left as they are.
4. **Minimap:** no code. `compute_minimap_colors` reads the top face of the shape with its
   tint, and darkens the transparent blocks by `CANOPY_SHADE`: the jungle leaves get both from
   their shape and their transparency, which the tests below pin.
5. **Recipes** go each right after its spruce twin in `RECIPES`; the order does not change
   what matches.
6. **World preview tools:** the three blocks join the palette of `worldgen_preview.py`, which
   is indexed by the variants of `BlockId`, and the log and leaves are not ground for
   `worldgen_dump.rs`, like the spruce ones. The jungle biome's own colour waits for lot 2,
   with the biome.
7. **`CHANGELOG.txt`** is left to lot 2: nothing in a world shows jungle wood before the biome.

## Tasks

### Task 1: Blocks and items (`shared`)
- [ ] Tests: add the three blocks to `block_encoding_is_pinned` (28, 29, 30), `ALL_BLOCKS` and
  its exhaustive match, and the three items to `item_encoding_is_pinned` (35, 36, 37). See
  them fail to compile.
- [ ] Append the variants, grow `BlockId::ALL` and `ItemId::ALL`, map each item to its block
  in `get_default_type`. See them pass.
- [ ] Tests: `jungle_wood_breaks_like_oak_wood` (break times, the axe for log and planks, the
  hoe and the sword for the leaves, no tool required), `jungle_wood_drops_itself_and_its_leaves_drop_nothing`
  and `jungle_leaves_are_seen_through_like_the_other_leaves`. See them fail.
- [ ] Break times, `is_mined_faster_by`, drop tables, `get_visibility`. See them pass.
- [ ] Fix the completion test that lists the logs: `/give LOG` now finds `jungle_log` too.
- [ ] `PROTOCOL_ID` 19 → 20.

### Task 2: Recipes (`shared`)
- [ ] Tests: `JUNGLE` planks in `a_log_makes_four_planks`, and in the loops over the planks of
  the sticks, crafting table, chest and wooden tools tests; mixed jungle and oak planks make no
  tool. See them fail.
- [ ] The nine jungle recipes. See them pass.

### Task 3: Client
- [ ] Tests: `jungle_wood_sounds_like_wood_and_its_leaves_like_leaves` (sounds.rs);
  `the_jungle_log_has_its_own_top_texture` and
  `jungle_leaves_are_tinted_on_every_face_by_their_own_colour` (voxel.rs). See them fail.
- [ ] `JUNGLE_LEAVES_COLOR`, the sound materials, the `VoxelShape` cases. See them pass.
- [ ] Textures from Pixel Perfection: `block/jungle_log`, `jungle_log_top`, `jungle_planks`,
  `jungle_leaves` → `blocks/JungleLog.png`, `JungleLogTop.png`, `JunglePlanks.png`,
  `JungleLeaves.png`, each checked 16×16.
- [ ] Test: `the_jungle_leaves_icon_is_its_block_texture_tinted_as_in_the_world`. See it fail.
- [ ] `items/JungleLeaves.png`, made by a script kept outside the repository. See it pass.

### Task 4: Tools, credits and commit
- [ ] `worldgen_preview.py` palette, `worldgen_dump.rs` `is_ground`.
- [ ] Credit the four textures in `docs/ressources.txt`.
- [ ] `cargo test`, clippy and fmt on `shared`, `client` and `server`; commit `feat: jungle
  logs, planks and leaves, and their recipes`, naming the textures and the pack.
