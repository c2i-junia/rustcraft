# Item Browser Lot 4 (Bookmarks) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `A` over any item slot bookmarks its item, or removes it from the bookmarks, with a toast. The bookmarks show as a `Bookmarks` label and a wrapping row of slots between the chips and the grid, only when there is one, and those slots behave as the catalog's. The recipe view says `· bookmarked`; the tooltips and the Survival footer name `A`. The client keeps the bookmarks in `item_browser.ron` in the game folder, one ordered list per world: by its name in solo, by the server's address in multiplayer.

**Architecture:** A new `item_browser/bookmarks.rs` holds the `Bookmarks` resource (the key of the world being played and its ordered list), the file (`ItemBrowserFile { worlds, servers }`, two maps of lists, `#[serde(default)]`), its tolerant reading and its writing, which keeps the lists of the other worlds, and the texts. `load_bookmarks` runs on `OnEnter(GameState::Game)` from `SessionOrigin` (`World { name }` → `worlds[name]`, `Server { ip, .. }` → `servers[ip]`) and `GameFolderPaths`, as `interface.ron` is read through `GameFolderPaths`; the toggle writes the file at once. The slot under the keys (focused while its ring shows, else hovered) is found by one `SystemParam`, `KeyedSlot`, shared by R / U (`item_keys`) and A (`bookmark_key`). The bookmark slots are catalog slots (`CatalogSlot` + `BookmarkSlot`), built by the function that builds the grid's, so that every gesture, tooltip, dot, Tab and arrow of the catalog reaches them; `filter_catalog` leaves them out of the search and the count. `show_bookmarks` rebuilds the row when the list changes and shows the bar (`BookmarkBar`) only when it is not empty and no view shows. No protocol change, and the browser still never writes `Res<Inventory>`.

**Tech Stack:** Rust nightly, Bevy `=0.19.1`, `ron`, `serde`, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-20-item-browser-design.md`, lot 4 ("Bookmarks"): the Bookmarks line, the footer and the tooltip of "What the player sees", `· bookmarked` of the recipe view, `A` in "Interactions", the last paragraph of "Client". The look: the approved mockup (`toggleBookmark`, `#bm` of `renderList`, `showTip`, the footer of `renderBrowser`).

## Global Constraints

The "Global Constraints" of the plans of lots 1 to 3 apply, with these readings for this lot:

- English for code, comments, docs, commit messages and game texts; Conventional Commits.
- Tests named as sentences; doc comments in full sentences.
- Every test command runs under `timeout`. `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit; the suite is green at the end of every task.
- E2e tests in `client/src/e2e/item_browser.rs`, `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, driven only through `click`/`hover`/`press`/`type_text`/menus. **One new journey**: bookmarks.
- Bookmark slots are `widgets::slot`s (hit test `Hovered`); decorations carry `Pickable::IGNORE`; the role before `AccessibleLabel`.
- **Inventory authority.** The browser never writes `Res<Inventory>`; bookmarks touch no inventory and no message: **no protocol change**.
- **Persistence** follows `ui/settings.rs`: a missing or broken `item_browser.ron` reads as no bookmark, and is overwritten by the next change.
- Goldens: the Survival goldens of the browser change **only in Task 2**, because the footer names `A` instead of `/` (the search field keeps its `/` cap); the new `item_browser_bookmarks_1280x720` comes in Task 3. The Creative golden, `hotbar.png` and every menu golden must not change. Each new or changed PNG is looked at before it is accepted.

## Review Focus

1. The file keeps every other world's list when one changes; a broken file never panics. (Task 1)
2. A world and a server with the same name or address do not share a list. (Task 1)
3. A acts on the same slot R and U act on, and not while the player types. (Task 2)
4. The bookmark slots are not counted or filtered by the search, and a focus on a slot the row loses moves to the same item. (Task 3)

## Files

- Create: `client/src/ui/hud/item_browser/bookmarks.rs` (Task 1).
- Modify: `client/src/constants.rs` (Task 1); `client/src/ui/hud/item_browser/{mod,setup,display,view,recipes,goldens}.rs`, `client/src/e2e/item_browser.rs`, `client/tests/golden/*.png` (Tasks 2–3).
- `README.md`, `docs/ARCHITECTURE.md`, the spec's "Decisions" and status (Task 4).

---

### Task 1: The bookmarks and their file

**Interfaces:**
```rust
pub const ITEM_BROWSER_PATH: &str = "item_browser.ron"; // constants.rs
#[derive(Clone, PartialEq, Eq, Debug)] pub enum BookmarkKey { World(String), Server(String) }
impl BookmarkKey { pub fn of(origin: &SessionOrigin) -> Self } // World{name} → World(name); Server{ip,..} → Server(ip)
#[derive(Serialize, Deserialize, Default, PartialEq, Debug)] #[serde(default)]
pub struct ItemBrowserFile { pub worlds: BTreeMap<String, Vec<ItemId>>, pub servers: BTreeMap<String, Vec<ItemId>> }
pub fn read_item_browser_file(paths: &GameFolderPaths) -> ItemBrowserFile;           // missing / broken → default
pub fn read_bookmarks(paths: &GameFolderPaths, key: &BookmarkKey) -> Vec<ItemId>;
pub fn write_bookmarks(paths: &GameFolderPaths, key: &BookmarkKey, items: &[ItemId]); // keeps the others; an empty list is removed
#[derive(Resource, Default, Clone, PartialEq, Debug)] pub struct Bookmarks { pub key: Option<BookmarkKey>, pub items: Vec<ItemId> }
impl Bookmarks { pub fn contains(&self, item) -> bool; pub fn toggle(&mut self, item) -> bool /* true: added, at the end */ }
pub fn bookmark_message(item: ItemId, added: bool) -> String; // "Bookmarked Chest" / "Removed Chest from bookmarks"
```
- [ ] **Step 1: Failing tests** (`bookmarks.rs`): `the_bookmarks_of_a_world_are_saved_and_read_back` (order kept); `each_world_and_each_server_has_its_own_bookmarks` (two worlds, a server named as a world, writing one keeps the others, an emptied list leaves the file); `a_missing_or_broken_file_has_no_bookmark` (missing, `garbage`, `()`, `(worlds: {"A": [Chest]})` without `servers`); `toggling_adds_at_the_end_and_removes` with the toasts; `the_key_of_a_session` (world name, server address as typed).
- [ ] **Step 2:** `timeout 600 cargo test -q -p client bookmarks` → fails to compile.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** tests pass, suite green.
- [ ] **Step 5: Commit** — `feat(client): the bookmarks of the item browser, kept per world in item_browser.ron`

### Task 2: A toggles a bookmark; the texts name it

- `Bookmarks` is a resource of the plugin; `load_bookmarks` (`OnEnter(GameState::Game)`) reads the list of `SessionOrigin` (none: no key, nothing read or written).
- `KeyedSlot` (`SystemParam`): the item of the focused slot while its ring shows, else of the hovered one, among the catalog, the cards, the inventory's cells (chest included) and the result. `item_keys` uses it; `bookmark_key` (after it in the chain): A, not typing, inventory open → `toggle`, `write_bookmarks`, a toast.
- `item_keys_hint(bookmarked) -> String`: `R Recipes  U Uses  A Bookmark` (`A Unbookmark` when it is, as the mockup's `showTip`); `mark_craftable_items` also runs when `Bookmarks` changes.
- `bookmarked_detail(item, bookmarked)`: the view's category line gains ` · bookmarked`; `show_view` also runs when `Bookmarks` changes.
- The Survival footer: `LMB recipes  RMB uses  A bookmark`, as the spec and the mockup (the search's `/` cap stays in the field).
- [ ] **Step 1: Failing tests:** `recipes.rs` `the_texts_of_the_view` (hint, detail); `display.rs` `a_bookmarked_item_says_so_in_its_tooltip`; e2e journey, first part: hover Chest, A → toast `Bookmarked Chest`, tooltip `A Unbookmark`; R → `Stations · bookmarked`; `item_browser.ron` holds `worlds: {"Bookmarks": [Chest]}`; A over an inventory cell holding a pickaxe → bookmarked too.
- [ ] **Step 2–4:** implement; Survival goldens change (footer): look, accept.
- [ ] **Step 5: Commit** — `feat(client): A bookmarks the item of any slot, and the browser's texts say so`

### Task 3: The bookmark row

- `spawn_catalog_slot(commands, parent, item, atlas, font) -> Entity` (setup.rs), used by the grid and the row.
- `spawn_bookmark_bar`: `BookmarkBar` (column, `display: None`), the label `Bookmarks` (FONT_S, MUTED), `BookmarkRow` (`Role::Grid`, label `Bookmarks`, wrapping, gaps `SPACE_XS`), between the chips and the grid.
- `show_bookmarks` (after `show_view`, when `Bookmarks` or the state changes): rebuilds the row's slots (`CatalogSlot(item)`, `BookmarkSlot`) when they differ from the list; a focus on a slot that goes moves to the new slot of its item, else to its grid slot, else clears; shows the bar when the list is not empty and no view shows. `filter_catalog` skips `BookmarkSlot`; `show_view` counts the bar as a part of the catalog for the focus.
- [ ] **Step 1: Failing tests:** golden `the_bookmark_row_at_1280x720` → `item_browser_bookmarks_1280x720` (Wooden Pickaxe and Chest, as the mockup); `display.rs` `bookmark_slots_are_neither_searched_nor_counted`; e2e journey, the rest: the row appears with Chest; a click on it opens its recipes (Survival); arrows reach it from the chips and go on to the grid; A on the focused bookmark slot removes it (row hidden), A again; save and quit, re-enter: the row holds Chest; another world: no row; back to the first: still there.
- [ ] **Step 2–4:** implement; read the new PNG; the others unchanged.
- [ ] **Step 5: Commit** — `feat(client): the bookmarks show as a row of slots above the catalog`

### Task 4: Docs and the final check

- README (Controls: A; Features; the saved files), `docs/ARCHITECTURE.md` (bookmarks.rs, item_browser.ron), the spec: Decisions 34+, the feature marked complete.
- [ ] `cargo fmt --all -- --check`; `timeout 900 cargo clippy --workspace --all-targets -- -D warnings`; `timeout 1500 cargo test --workspace --no-fail-fast` green; no `.actual.png` / `.diff.png` left.
- [ ] **Commit** — `docs: the bookmarks of the item browser in the README, the architecture and the decisions`
