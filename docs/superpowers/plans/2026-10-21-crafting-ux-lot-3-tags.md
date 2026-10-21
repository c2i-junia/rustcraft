# Crafting UX Lot 3 (Ingredient families) Implementation Plan

**Goal:** A recipe that takes planks takes any planks, as in Minecraft: species mix freely in
the grid, one recipe stands for the eight that were repeated per species (sticks, crafting
table, chest, the five wooden tools), the recipe book shows one card per recipe whose planks
cell cycles through the species once a second, says `Missing 3 Planks` and names the cell
`Any Planks`, and `+ Fill grid` takes one species when one is enough, else mixes them. Fixes
C8. To see: with 5 oak and 4 spruce planks at a crafting table, open the recipes of the chest
(one card), fill the grid and take the chest.

**Spec:** `docs/superpowers/specs/2026-10-21-crafting-ux-design.md`, "Global constraints",
"Task 3 — Lot 3: ingredient families", and its "Decisions" (lots 1 and 2 bind this lot).

## Global constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- The wire's encoding does not change, but the recipe indices do (41 recipes instead of 57):
  `PROTOCOL_ID` stays 25, bumped once for the branch in lot 1.
- Every rule stays in `shared`: matching, counting, and the fill's plan. The client's card,
  note and tint call the same functions.
- Goldens of the recipe views are regenerated with `UPDATE_GOLDEN=1`, and every changed PNG
  is looked at.
- `cargo test -p shared <filter>` / `cargo test -p client <filter>` while working;
  `timeout 1500 just check` at the end.

## Decisions (where the spec is silent)

1. **Types.** In `shared/src/world/crafting.rs`:
   `pub enum ItemTag { Planks }` with `members(self) -> &'static [ItemId]` (Oak, Spruce,
   Jungle, the order of `ItemId::ALL`) and `name(self) -> &'static str` (`"Planks"`);
   `pub enum Ingredient { Item(ItemId), Tag(ItemTag) }` with `matches(self, item) -> bool`
   and `members(self) -> &'static [ItemId]` (an item alone for `Item`). Both `Copy, Eq,
   Debug`. A test pins that every item whose name ends in `_planks` is a member: a new
   species is listed once, in `members`.
2. **Table.** Each collapsed family takes the place of its oak recipe, the spruce and jungle
   copies go: 41 recipes. The sticks stay index 3. `every_recipe_is_listed...` builds each
   recipe's grid from the first member of each cell.
3. **Views.** `RecipeView::cells` is `&'static [Option<Ingredient>]`; `ingredients()` and
   `missing()` give `Vec<(Ingredient, u32)>`. `uses_of(item)` is every recipe with a cell
   `item` matches. A tag counts the stacks of all its members together. No recipe has two
   ingredients sharing an item (pinned by a test), so no stack is counted twice.
4. **Fill.** `pub fn plan_fill(inventory, recipe, times) -> Option<FillPlan>` in
   `crafting.rs`, `FillPlan { rounds, cells }` (`cells`: the item of each cell of the recipe,
   row after row), counted on the slots `0..36`: the single computation of the fill, which
   lot 5's Ctrl+click can call. The rounds are the largest `r` ≤ `times` and ≤ 64 for which
   every cell, in order, can get `r` of one item that stacks to `r` at least. A tag cell takes
   the member the previous cell of the same tag took while it still holds `r`, else the
   member the inventory holds the most of after the cells already served, the first member on
   a tie. The spec's "the most of" alone would alternate species on a tie (8 oak and 8 spruce
   for a chest gives 4 and 4) where one is enough; staying with the member breaks that and
   finds the same rounds (a tag's cells get `r` each as long as the sum of `count / r` over
   its members reaches them, whatever the order). `fill_crafting_grid` puts the grid back,
   calls it, takes each cell's item as today.
5. **Client names.** `ingredient_name(Ingredient)`: an item's name, a tag's `name()`:
   `Missing 3 Planks, 2 Stick`. A tag cell's accessible label and tooltip title are
   `ingredient_label` = `Any Planks`; its detail is the category of its first member.
6. **Tag cell.** A card's tag cell is a `RecipeSlot` whose `item` is the member it shows,
   with a `TagCell(ItemTag)`: a click, R, U and A act on the member shown, as on any slot.
   Its tooltip note says `You have the ingredients` when some member is craftable now. Its
   `MissingTint` carries the `Ingredient`, shown while the tag as a whole is short.
7. **Cycle.** `tag_member(tag, elapsed)` = `members[whole seconds % len]`, on `Time<Real>`:
   every tag cell of every card is in step. `cycle_tag_cells` (in the browser's chain, after
   `show_view`) sets the slot's item and icon when the second changes the member. A card
   spawns showing the first member, which the cycle corrects in the same frame. The goldens do
   not run the cycle: they show the first member, Oak, whatever time it is.
8. **Tooltips follow.** `mark_craftable_items` runs on `Changed<RecipeSlot>` instead of
   `Added<RecipeSlot>`, so that the keys' `A Bookmark` / `A Unbookmark` follow the member
   shown.

## Steps

### 1. Shared: tags and matching

- RED: tests in `crafting.rs`:
  - `planks_of_any_kind_one_above_the_other_make_four_sticks`, replacing
    `mixed_planks_make_nothing` (now its opposite: oak above spruce makes sticks);
  - `a_tool_takes_planks_of_any_kind` replacing `a_tool_needs_planks_of_one_kind`;
  - `a_ring_of_planks_makes_a_chest` with a mixed ring;
  - `every_recipe_with_planks_is_made_by_each_species`: for each recipe with a tag cell,
    each member put in every tag cell makes the output; mirrored too;
  - `the_planks_tag_holds_every_planks_item`, `an_ingredient_matches_its_items`;
  - `every_recipe_is_listed_with_its_index_and_size`: 41, from first members;
  - `recipes_for_and_uses_of`: `WoodenPickaxe (1, 0)`, `Stick (1, 27)`, `OakPlanks (1, 8)`,
    `SprucePlanks (1, 8)`, the same recipes for each species;
  - `the_ingredients_of_a_recipe_are_counted_by_cell`: `[(Tag(Planks), 3), (Item(Stick), 2)]`;
  - `missing_counts_every_member_of_a_tag_together`: one oak, one spruce in the inventory, one
    jungle in the grid: only the sticks are missing; with two: `[(Tag(Planks), 1), ...]`;
  - `no_recipe_has_two_ingredients_sharing_an_item`.
- GREEN: `ItemTag`, `Ingredient`, the table, `match_recipe`, `RecipeView`, `ingredients`,
  `uses_of`, `missing`. Fix the compile of `inventory.rs`'s fill (still one item per cell:
  the first member) so the crate builds; the client is fixed in step 3.
- Commit `feat(shared): a recipe cell takes any planks`.

### 2. Shared: the fill's plan

- RED: in `crafting.rs`, `plan_fill` tests: one species when it is enough (10 oak, 3 spruce:
  8 oak for a chest), the first on a tie (8 and 8: oak), the one with the most (8 oak, 9
  spruce: spruce), a mix when none is enough (5 oak, 4 spruce: 5 oak then 3 spruce, in cell
  order), the largest rounds (20 oak, 10 spruce, `u32::MAX`: 3 rounds, 6 oak cells and 2
  spruce), clamped to 64 and `times`, `None` without one round. In `inventory.rs`:
  `filling_the_grid_takes_one_species_when_it_is_enough`,
  `filling_the_grid_mixes_species_when_none_is_enough` (the grid's cells and what stays).
- GREEN: `FillPlan`, `plan_fill`, and `fill_crafting_grid` on it.
- Commit `feat(shared): filling the grid takes one species of planks when it is enough`.

### 3. Client: names, problems, cards

- RED: `recipes.rs`: `recipe_problem` gives `Missing 3 Planks, 2 Stick`, and counts mixed
  species; `ingredient_name`, `ingredient_label`; `cards_of` the wooden pickaxe is one card,
  the uses of spruce planks list the tag recipes then its burning; the uses counts of the
  empty-view messages. `view.rs`: the tint test with a tag (`MissingTint` with the
  `Ingredient`), `tag_member` and `cycle_tag_cells` with a `Time<Real>` stepped by hand (two
  cells in step, the member changing each second, wrapping), the tag cell's tooltip.
- GREEN: `ingredient_name`, `ingredient_label`, `MissingTint { recipe, ingredient }`,
  `TagCell`, `spawn_card` with tag cells, `cycle_tag_cells` in the plugin's chain after
  `show_view`, `mark_craftable_items` on `Changed<RecipeSlot>` with the tag note.
- Commit `feat(client): one recipe card for any planks, its planks cycling`.

### 4. Goldens and e2e

- Goldens: the count test (one card, 6 slots, 5 tints of which 2 show). Regenerate
  `the_recipes_of_the_wooden_pickaxe_at_1280x720`, `the_uses_of_oak_planks_at_854x480` and any
  other recipe view that changed with `UPDATE_GOLDEN=1`, look at each PNG.
- e2e: update the counts of `item_browser.rs` (`Recipes 1` for the pickaxe, the stick and the
  chest; `Uses 28` for the stick; the `Any Planks` slots). New
  `fill_grid_mixes_the_species_of_planks_at_a_table`: 5 oak, 4 spruce, a table placed and
  opened; the chest's one card, its eight `Any Planks` cells; `+ Fill grid` puts 5 oak then 3
  spruce in the ring, one spruce stays; the result is a chest, taken onto the cursor.
- Commits `test(client): goldens of the recipe cards of any planks`,
  `test(client): e2e of filling the grid with mixed planks`.

### 5. Wrap-up

- Append the rulings to the spec's Decisions, `(Lot 3)`, from 22.
- `timeout 1500 just check`; re-run a failing test alone before calling it a flake.
- Commit `docs: decisions of the ingredient families (lot 3)`; write the report.
