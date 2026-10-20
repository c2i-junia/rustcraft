# Jungle Lot 5 (Cocoa) Implementation Plan

**Goal:** The cocoa: a pod hung 1/16 off the side of a jungle log, broken in two steps for two
cocoa beans, and generated against the top of the trunk of one jungle tree in three. To see:
brown pods under the crowns of the jungle trees, broken for their beans.

**Spec:** `docs/superpowers/specs/2026-10-20-jungle-biome-design.md`, the cocoa parts of the rows
"Jungle tree", "New blocks", "New items", "Block behaviour", "Textures" and "Protocol" of
"Decisions", the "Generation" and "Client" paragraphs of "Architecture", and lot 5 of "Lots".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `BlockId` and `ItemId` are append only; `PROTOCOL_ID` stays 20 (bumped once in lot 1).
- Generation stays a pure function of `(seed, coordinates)`; each new random decision takes a
  salt of its own, never one already used.
- `STRUCTURE_MARGIN` stays 3 and `GIANT_MARGIN` 6; `Decoration::height` does not change; no
  structure block lies below its root but the giant's sunk trunk.
- Reuse lot 4: the direction names the side of the cell the support lies on
  (`BlockDirection::offset`, `toward`, `turn`), `BlockData::get_ray_hitbox`, the `Blocks`
  triples with a direction.
- `cargo test`, `cargo clippy -p shared -p client -p server --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at each commit.

## Decisions (where the spec is silent)

1. **Variants:** `BlockId::Cocoa` is 33, `ItemId::CocoaBeans` 40.
2. **Shape:** the pack's model `cocoa_stage2.json` has a pod box from (4, 3, 7) to (12, 12, 15)
   and a stem, a square in the plane x = 8 from (8, 12, 12) to (8, 16, 16), both held on the
   south (+Z), which Minecraft's blockstate leaves unturned for `facing=south`. Our `Front`
   cocoa is held on the north (−Z): the model turned half a turn, as Minecraft draws
   `facing=north`. So the pod spans x 4..12, y 3..12, z 1..9 sixteenths, 1/16 off the log, and
   the stem reaches the log at the top of the cell. Each face takes the UVs of the model on
   `Cocoa.png` with Minecraft's mapping (`u` from the left of the face seen from outside, `v`
   from its top; in the model, the top face's top toward the north, the bottom's toward the
   south): the four sides (8, 4)–(15, 13), the top and bottom (0, 0)–(7, 7), the stem
   (12, 0)–(16, 4), mirrored on its east side as the model says. The stem is two back to back squares: kept.
   Every face is `Inset`, never culled, untinted.
3. **Visibility:** `Decoration`: it hides no face of its neighbours, like the vine. It also
   means, as for the plants: water washes it away (as in Minecraft), a tool breaking it is not
   worn, and the minimap sees through it.
4. **Boxes:** the collision box and the box rays hit are the pod's box, without the stem,
   turned to the direction of the block. `BlockData::get_hitbox` joins
   `BlockData::get_ray_hitbox`, which `ChunkMap::check_collision_box` now reads; both turn
   the box of every block (nothing for a whole block or the centred boxes of the plants).
5. **Breaking:** 2 steps, no tool required, faster with an axe (Minecraft mines it with one);
   drops 2 cocoa beans (one entry of the drop table, `(1, CocoaBeans, 2)`).
6. **Sound:** wood, like Minecraft's cocoa: a hard shell held on a log.
7. **The beans:** a generic item, stacked by 64, with no use yet; nothing places a cocoa block
   (no item does, the beans are generic and there is no `cocoa` item to `/give`): it only
   grows from the generation. Icon `item/cocoa_beans.png` → `items/CocoaBeans.png`.
8. **Generation:** a jungle tree's trunk is at least 5 logs, its crown starts 2 under its top
   log. The cells beside the trunk 3 and 4 under its top (the 2 logs just under the crown, never
   under the root) on each of its 4 sides are the 8 cells a pod may take. One roll,
   `below(SALT_COCOA, 6)`: 0 gives one pod, 1 two, anything else none (1 tree in 3). The
   first pod takes the cell `below(SALT_COCOA + 1, 8)`, the second the one
   `1 + below(SALT_COCOA + 2, 7)` further in that order, never the same. Each turned toward
   the trunk. Salts `0x470` to `0x472`.
9. **Rank:** with the logs, as the spec says (4): leaves, vines and plants never take its
   cell, the terrain does. Two jungle trees side by side may then leave the first placed of a
   pod and a log of the other in a cell; the generation test counts how often on the seed 42.
10. **Margins:** a pod is within 1 column of its trunk, under the crown: no margin nor height
    bound moves.
11. **Preview tools:** `worldgen_dump`'s `is_ground` skips the cocoa, `worldgen_preview.py`
    colours it like its texture, orange brown.
12. **`CHANGELOG.txt`:** the jungle line mentions the cocoa pods and their beans.

## Tasks

### Task 1: The cocoa block and the beans (`shared`, `client`)
- [x] Tests: `Cocoa` in `block_encoding_is_pinned` (33), `ALL_BLOCKS`; `CocoaBeans` in
  `item_encoding_is_pinned` (40); `the_cocoa_is_a_pod_broken_in_two_steps_for_two_beans`
  (visibility, break time, axe, no tool required, drops, the beans generic);
  `a_cocoa_pod_is_boxed_where_it_hangs` (both boxes, each direction: 8 wide, 9 high, 8 deep,
  1/16 off the support, centred along it). See them fail.
- [x] The variants, their behaviour, `BlockData::get_hitbox` and the turned boxes. See them
  pass.
- [x] Test: `a_cocoa_pod_collides_only_through_its_box` (chunk_map.rs, each direction). See it
  fail; read `BlockData::get_hitbox` in `check_collision_box`. See it pass. Rays already read
  `BlockData::get_ray_hitbox`, as for the vine: no other test there.
- [x] Tests: `the_cocoa_sounds_like_wood`;
  `the_cocoa_is_the_pod_of_its_model_one_sixteenth_off_its_support` (voxel.rs: 8 `Inset`
  faces, untinted, wound toward their normals, the pod box and the stem where decided 2, the
  UVs of the model); in meshing.rs
  `a_cocoa_pod_hangs_against_the_log_that_holds_it_and_hides_none_of_its_faces` (each
  direction: in its cell, its face toward the log 1/16 off it, the stem touching it, every face
  of the log drawn). See them fail. The shape and the sound. See them pass.
- [x] Textures `block/cocoa_stage2.png` → `blocks/Cocoa.png`, `item/cocoa_beans.png` →
  `items/CocoaBeans.png` (16 × 16).
- [x] Credit the textures; commit `feat: the cocoa`.

### Task 2: Cocoa on the jungle trees (`server`)
- [ ] Tests (jungle.rs): `one_jungle_tree_in_three_holds_one_or_two_pods_under_its_crown`
  (the pods beside the trunk 3 or 4 under its top, turned toward it, distinct, the share of
  trees near 1/3, one and two pods both seen, every one of the 8 cells seen); the crown test
  still sees no leaf outside the crown. See it fail.
- [ ] Tests (generation.rs): the cocoa in `jungle_blocks_rank_with_their_oak_and_grass_siblings`;
  `pods_hang_on_jungle_logs_in_the_jungle` (sample jungle chunks of the seed 42: pods found,
  each with a jungle log on its support's side, none with a log above and below it, which would
  be a pod in the trunk of another tree). See them fail.
- [ ] The pods of `jungle_tree_blocks`, the rank. See them pass, with the determinism, margin,
  culling and completeness tests.
- [ ] `worldgen_dump.rs`, `worldgen_preview.py`, `CHANGELOG.txt`; tests, clippy, fmt; commit
  `feat: cocoa on the jungle trees`.

### Task 3: A picture of the pods in the game
- [ ] An ignored `UI_SNAPSHOTS=1` e2e snapshot of a jungle tree of the seed 42 with pods, close
  up, looked at and kept outside the repository (`cocoa_*`); commit
  `test(client): pictures of the cocoa pods of a jungle tree`.
- [ ] The whole workspace: `cargo test -p shared -p server -p client`, clippy, fmt.
