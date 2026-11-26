// The world settings the agent puts on a server's command line, copied from the game, as its
// server's flags take them (`server/src/main.rs`). The server's tests include this file and
// check it against the game, so that it follows when the game changes.

/// `--world-type`: `WorldPreset::NAMES` (`shared/src/world/terrain.rs`) in kebab-case, but
/// `single-biome`, which needs a `--biome` that the settings of a hosted world do not carry.
pub const WORLD_TYPES: [&str; 3] = ["default", "large-biomes", "archipelago"];

/// `--game-mode`: `GameMode::name` (`shared/src/players/game_mode.rs`).
pub const GAME_MODES: [&str; 2] = ["survival", "creative"];

/// `--difficulty`: `Difficulty::name` (`shared/src/world/hostiles.rs`).
pub const DIFFICULTIES: [&str; 4] = ["peaceful", "easy", "normal", "hard"];
