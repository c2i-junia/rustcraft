use bevy::prelude::*;

/// Bump it whenever a network message or an enum sent in one (`BlockId`, `BlockDirection`,
/// `ItemId`, ...) changes, so that a client and a server of different builds refuse to talk
/// instead of misreading each other.
pub const PROTOCOL_ID: u64 = 24;
/// The UDP port a server listens on when none is given, and that an address without one means.
pub const DEFAULT_PORT: u16 = 8000;
pub const TICKS_PER_SECOND: u64 = 20;
/// Length of a day and night cycle: twenty minutes, as in Minecraft. A cycle starts at sunset,
/// so its first half is the night (`client/src/world/celestial.rs`). Bump `PROTOCOL_ID` with
/// it: a client and a server must see the sun at the same place.
pub const DAY_DURATION_IN_TICKS: u64 = TICKS_PER_SECOND * 60 * 20;
/// When the sun is at its highest, counted from the start of a cycle.
pub const NOON_IN_DAY: u64 = DAY_DURATION_IN_TICKS * 3 / 4;
/// When the moon is at its highest, counted from the start of a cycle.
pub const MIDNIGHT_IN_DAY: u64 = DAY_DURATION_IN_TICKS / 4;
pub const CHUNK_SIZE: i32 = 16;
/// The chunks within a player's render distance (`chunk_distance`) are generated and sent to
/// its client. Each player asks for its own, which the server keeps between
/// `MIN_RENDER_DISTANCE` and its maximum.
pub const MIN_RENDER_DISTANCE: u32 = 1;
/// The largest render distance a server grants when it is not told otherwise: all a client may
/// ask for. The chunks within 64 are 129² columns of 17 chunks
/// (`world::MAX_VERTICAL_RENDER_DISTANCE`), 282897 per player.
pub const DEFAULT_MAX_RENDER_DISTANCE: u32 = 64;
/// The client waits for the chunks within this many chunks of its spawn before entering the
/// game. Every render distance reaches it, and the nearest chunks come first: bump
/// `PROTOCOL_ID` with it.
pub const SPAWN_CHUNK_RADIUS: i32 = MIN_RENDER_DISTANCE as i32;
/// A chunk a client has is unloaded once it is more than its render distance plus
/// `UNLOAD_MARGIN` chunks from its player, so that walking to and fro across a chunk border
/// neither unloads nor sends anything.
pub const UNLOAD_MARGIN: i32 = 1;
pub const MAX_INVENTORY_SLOTS: u32 = 4 * 9;
/// The first slots of the inventory, the ones the hotbar shows.
pub const HOTBAR_SLOTS: u32 = 9;
pub const HALF_BLOCK: Vec3 = Vec3 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
};

/// Most inputs the server accepts from one client in one tick; the excess is dropped.
///
/// A client records one input per rendered frame and uploads them at the tick rate (20 Hz), so
/// a message holds the frames of 50 ms: 3 at 60 fps, 12 at 240 fps, 50 at 1000 fps. The
/// messages a stalled server received meanwhile are read together in its next tick: 512 covers
/// a two second stall of a 240 fps client, and bounds the work one client can ask per tick to
/// what a 10000 fps client would.
pub const MAX_INPUTS_PER_TICK: usize = 512;

/// Most inventory actions one input may carry; the excess is ignored. A person clicks a few
/// times per frame at most, and an action costs a few slot lookups: this bounds what a client
/// can ask per input, like `MAX_INPUTS_PER_TICK` bounds the inputs.
pub const MAX_INVENTORY_ACTIONS_PER_INPUT: usize = 32;
