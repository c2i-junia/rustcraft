mod auth;
mod chat;
pub mod mob;
pub mod player;
mod world;

use crate::players::progress::{Achievement, FirstStep, Progress};
use crate::world::cartography::{MapId, PaperMap, WaypointsView};
use crate::world::{
    depths::DepthTier, MobId, MobKind, Portal, PortalColor, TradeView, WeatherKind,
};
pub use auth::*;
use bevy::math::{IVec3, Vec3};
pub use chat::*;
use mob::MobUpdateEvent;
pub use player::*;
use serde::{Deserialize, Serialize};
pub use world::*;

pub type PlayerId = u64;

/// Append only, like `ServerToClientMessage` and `NetworkAction`: a message starts with the
/// index of its variant. Bump `PROTOCOL_ID` with any change.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientToServerMessage {
    AuthRegisterRequest(AuthRegisterRequest),
    ChatMessage(ChatMessageRequest),
    Exit,
    PlayerInputs(Vec<PlayerFrameInput>),
    SaveWorldRequest,
    /// The player changed its render distance, in chunks.
    SetRenderDistance(u32),
    /// The player, dead, asks to come back at the spawn.
    Respawn,
    /// The player takes the offer of this index of the villager it trades with.
    Trade {
        mob: MobId,
        offer: u8,
    },
    /// The player closed the trade screen.
    EndTrade,
    /// The player did this first step, or left it: its hint never shows again.
    FirstStep(FirstStep),
    /// The player wrote these lines on the sign at `at`, in the editor `EditSign` opened.
    WriteSign {
        at: IVec3,
        lines: [String; SIGN_LINES],
    },
    /// Names the place `at` on the world map (`cartography::Waypoints::mark`).
    PlaceMarker {
        at: IVec3,
        name: String,
        color: u8,
    },
    /// Takes away the marker of this index.
    RemoveMarker(u16),
}

/// The lines of a sign.
pub const SIGN_LINES: usize = 4;
/// The most characters a line of a sign holds.
pub const SIGN_LINE_LENGTH: usize = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerToClientMessage {
    AuthRegisterResponse(AuthRegisterResponse),
    ChatMessage(FullChatMessage),
    WorldUpdate(WorldUpdate),
    BlockUpdates(Vec<BlockUpdate>),
    PlayerSpawn(PlayerSpawnEvent),
    MobUpdate(MobUpdateEvent),
    PlayerUpdate(PlayerUpdateEvent),
    /// The chunks the client must forget: its player went too far from them. The server
    /// forgets it sent them, and sends them again once the player comes back.
    ChunkUnloads(Vec<IVec3>),
    DroppedItems(DroppedItemsUpdate),
    /// The server's clock jumped to this tick (`/time set`).
    TimeSet(u64),
    /// This mob is gone for this client: out of its reach, or removed.
    MobRemoved(MobId),
    /// The weather changed to this one, by itself or by `/weather`.
    WeatherSet(WeatherKind),
    /// A lightning strike hit the block at `at`. `seed` shapes the bolt, so that every player
    /// sees the same one.
    LightningStrike {
        at: IVec3,
        seed: u32,
    },
    /// Every portal of the world: sent to everyone when one changed, and to a player when it
    /// joins. The client's portals become these.
    PortalUpdates(Vec<Portal>),
    /// A shot of the portal gun by `shooter` (`players::blocks::PortalShot`), sent to every
    /// player but the shooter, who predicted it, before the `PortalUpdates` it may cause.
    PortalShot {
        shooter: PlayerId,
        from: Vec3,
        to: Vec3,
        color: PortalColor,
        placed: bool,
    },
    /// The server dismisses this client, for this reason shown to the player: kicked or banned
    /// by an operator, or refused when it asked to join (banned, not on the whitelist). The
    /// server disconnects it a moment later.
    Dismissed(String),
    /// The player of this client reached this depth tier for the first time.
    DepthReached(DepthTier),
    /// The trade of the player: what its villager has left, or none once it ended.
    TradeUpdate(Option<TradeView>),
    /// The health of the boss awake near the player of this client, none once there is none.
    BossBar(Option<BossBar>),
    /// The boss was beaten while the player of this client was in its lair: the credits.
    Victory,
    /// What the player of this client achieved, when it joins and when it changes, with what it
    /// has just earned.
    Progress {
        progress: Progress,
        earned: Vec<Achievement>,
    },
    /// Sent on connection, before anything else: what a player of a hub signs to prove its
    /// account (`AuthRegisterRequest::proof`). `hub` is the URL of the hub whose accounts the
    /// server takes, none for a server in offline mode.
    AuthChallenge {
        nonce: [u8; 32],
        server_key: [u8; 32],
        hub: Option<String>,
    },
    /// The player of this client placed the sign at `at`, or clicked it: the client opens the
    /// editor of its lines, which holds what it says.
    EditSign {
        at: IVec3,
        lines: [String; SIGN_LINES],
    },
    /// The waypoints of the player, sent when they change.
    Waypoints(WaypointsView),
    /// The cells of a paper map the player holds or may see in a frame, sent when they change.
    PaperMap {
        id: MapId,
        map: PaperMap,
    },
    /// Every frame of a map of the world that holds one, and the id of its map.
    FramedMaps(Vec<(IVec3, MapId)>),
    /// What every item frame and flower pot of the world shows, and the patterns of its banners
    /// (`decor`), sent when they change.
    Decorations(crate::world::Decorations),
}

/// What the bar of a boss shows.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossBar {
    pub kind: MobKind,
    pub health: u32,
    pub max_health: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::{DeathCause, GameMode};
    use crate::players::inventory::InventoryAction;
    use crate::world::{FaceDirection, MobAction, MobKind, MobTarget, PortalColor, WeatherKind};
    use bevy::math::{IVec3, Vec3};

    const ENCODING_WARNING: &str = "the order of the variants is the wire format: reordering \
        them makes a client and a server misread each other. New variants go at the END of the \
        enum; then add them to this table and bump PROTOCOL_ID.";

    /// The index a message starts with on the wire.
    fn variant_index<T: Serialize>(value: &T) -> u8 {
        use bincode::Options;
        bincode::options().serialize(value).unwrap()[0]
    }

    #[test]
    fn client_to_server_variants_are_pinned() {
        let request = AuthRegisterRequest {
            username: String::new(),
            render_distance: 0,
            skin: Default::default(),
            proof: None,
        };
        let chat = ChatMessageRequest {
            content: String::new(),
        };
        for (message, expected) in [
            (ClientToServerMessage::AuthRegisterRequest(request), 0),
            (ClientToServerMessage::ChatMessage(chat), 1),
            (ClientToServerMessage::Exit, 2),
            (ClientToServerMessage::PlayerInputs(vec![]), 3),
            (ClientToServerMessage::SaveWorldRequest, 4),
            (ClientToServerMessage::SetRenderDistance(0), 5),
            (ClientToServerMessage::Respawn, 6),
            (ClientToServerMessage::Trade { mob: 0, offer: 0 }, 7),
            (ClientToServerMessage::EndTrade, 8),
            (ClientToServerMessage::FirstStep(FirstStep::Move), 9),
            (
                ClientToServerMessage::WriteSign {
                    at: IVec3::ZERO,
                    lines: Default::default(),
                },
                10,
            ),
            (
                ClientToServerMessage::PlaceMarker {
                    at: IVec3::ZERO,
                    name: String::new(),
                    color: 0,
                },
                11,
            ),
            (ClientToServerMessage::RemoveMarker(0), 12),
        ] {
            assert_eq!(variant_index(&message), expected, "{ENCODING_WARNING}");
        }
    }

    #[test]
    fn server_to_client_variants_are_pinned() {
        let response = AuthRegisterResponse {
            username: String::new(),
            session_token: 0,
            tick: 0,
            timestamp_ms: 0,
            players: vec![],
            max_render_distance: 0,
            seed: 0,
            generation: Default::default(),
            biomes: vec![],
            mod_commands: vec![],
            weather: WeatherKind::Clear,
        };
        let chat = FullChatMessage {
            content: String::new(),
            author: String::new(),
            timestamp: 0,
        };
        let spawn = PlayerSpawnEvent {
            id: 0,
            name: String::new(),
            data: PlayerSave::default(),
            skin: Default::default(),
        };
        let mob = MobUpdateEvent {
            id: 0,
            mob: crate::world::ServerMob {
                kind: MobKind::Fox,
                target: MobTarget::None,
                action: MobAction::Walk,
                position: Vec3::ZERO,
                rotation: Default::default(),
                height: 1.0,
                width: 1.0,
                on_ground: true,
                velocity: Vec3::ZERO,
                depth: 1.0,
                action_ticks: 0,
                damage: 0,
                sheared: false,
                passes: 0,
                growing: 0,
                in_love: 0,
                owner: None,
                tier: None,
                breed_wait: 0,
                portal_rest: None,
                hurt_ticks: 0,
                attack_wait: 0,
                extra_health: 0,
                arrow: None,
                route: None,
                bobber: None,
                villager: None,
                struck: None,
                horse: None,
                saddled: false,
                riders: [None; 2],
                parries: 0,
                guard_down: 0,
                sitting: false,
                horse_armour: None,
                pack: None,
                leash: None,
                prey: None,
                potion: None,
            },
        };
        let update = PlayerUpdateEvent {
            id: 0,
            movement: Default::default(),
            orientation: Default::default(),
            last_ack_time: 0,
            inventory: Default::default(),
            open_chest: None,
            open_furnace: None,
            health: 0,
            air: 0,
            food: 0,
            experience: Default::default(),
            burning_ticks: 0,
            game_mode: Default::default(),
            death_cause: None,
            swinging: false,
            held: None,
            worn: Default::default(),
            effects: Default::default(),
            parries: 0,
        };
        for (message, expected) in [
            (ServerToClientMessage::AuthRegisterResponse(response), 0),
            (ServerToClientMessage::ChatMessage(chat), 1),
            (
                ServerToClientMessage::WorldUpdate(WorldUpdate::default()),
                2,
            ),
            (ServerToClientMessage::BlockUpdates(vec![]), 3),
            (ServerToClientMessage::PlayerSpawn(spawn), 4),
            (ServerToClientMessage::MobUpdate(mob), 5),
            (ServerToClientMessage::PlayerUpdate(update), 6),
            (ServerToClientMessage::ChunkUnloads(vec![]), 7),
            (
                ServerToClientMessage::DroppedItems(DroppedItemsUpdate::default()),
                8,
            ),
            (ServerToClientMessage::TimeSet(0), 9),
            (ServerToClientMessage::MobRemoved(0), 10),
            (ServerToClientMessage::WeatherSet(WeatherKind::Rain), 11),
            (
                ServerToClientMessage::LightningStrike {
                    at: IVec3::ZERO,
                    seed: 0,
                },
                12,
            ),
            (ServerToClientMessage::PortalUpdates(vec![]), 13),
            (
                ServerToClientMessage::PortalShot {
                    shooter: 0,
                    from: Vec3::ZERO,
                    to: Vec3::ZERO,
                    color: PortalColor::Blue,
                    placed: false,
                },
                14,
            ),
            (ServerToClientMessage::Dismissed(String::new()), 15),
            (ServerToClientMessage::DepthReached(DepthTier::Caves), 16),
            (ServerToClientMessage::TradeUpdate(None), 17),
            (ServerToClientMessage::BossBar(None), 18),
            (ServerToClientMessage::Victory, 19),
            (
                ServerToClientMessage::Progress {
                    progress: Default::default(),
                    earned: vec![],
                },
                20,
            ),
            (
                ServerToClientMessage::AuthChallenge {
                    nonce: [0; 32],
                    server_key: [0; 32],
                    hub: None,
                },
                21,
            ),
            (
                ServerToClientMessage::EditSign {
                    at: IVec3::ZERO,
                    lines: Default::default(),
                },
                22,
            ),
            (ServerToClientMessage::Waypoints(Default::default()), 23),
            (
                ServerToClientMessage::PaperMap {
                    id: 0,
                    map: PaperMap::new(Default::default()),
                },
                24,
            ),
            (ServerToClientMessage::FramedMaps(vec![]), 25),
            (ServerToClientMessage::Decorations(Default::default()), 26),
        ] {
            assert_eq!(variant_index(&message), expected, "{ENCODING_WARNING}");
        }
    }

    #[test]
    fn network_actions_and_mob_enums_are_pinned() {
        for (action, expected) in [
            (NetworkAction::MoveForward, 0),
            (NetworkAction::MoveRight, 1),
            (NetworkAction::MoveBackward, 2),
            (NetworkAction::MoveLeft, 3),
            (NetworkAction::JumpOrFlyUp, 4),
            (NetworkAction::SneakOrFlyDown, 5),
            (NetworkAction::ToggleFlyMode, 6),
            (NetworkAction::LeftClick, 7),
            (NetworkAction::RightClick, 8),
            (NetworkAction::Sprint, 9),
            (NetworkAction::LowerBow, 10),
        ] {
            assert_eq!(variant_index(&action), expected, "{ENCODING_WARNING}");
        }
        for (action, expected) in [
            (InventoryAction::LeftClickSlot(0), 0),
            (InventoryAction::RightClickSlot(0), 1),
            (InventoryAction::Close, 2),
            (InventoryAction::TakeCraftResult, 3),
            (InventoryAction::OpenCraftingTable(IVec3::ZERO), 4),
            (InventoryAction::QuickMoveSlot(0), 5),
            (InventoryAction::SwapWithHotbar { slot: 0, hotbar: 0 }, 6),
            (InventoryAction::OpenChest(IVec3::ZERO), 7),
            (
                InventoryAction::TakeFromCatalog {
                    item: crate::world::ItemId::Dirt,
                    count: 0,
                    to_inventory: false,
                },
                8,
            ),
            (InventoryAction::DestroyFloating, 9),
            (
                InventoryAction::FillCraftingGrid {
                    recipe: 0,
                    times: 0,
                },
                10,
            ),
            (InventoryAction::OpenFurnace(IVec3::ZERO), 11),
            (
                InventoryAction::Drag {
                    slots: 0,
                    one_each: false,
                },
                12,
            ),
            (InventoryAction::CraftAll, 13),
            (InventoryAction::CollectAll, 14),
            (
                InventoryAction::MoveAllOfType {
                    slot: 0,
                    item: crate::world::ItemId::Dirt,
                },
                15,
            ),
            (InventoryAction::CraftToHotbar(0), 16),
            (
                InventoryAction::DropSlot {
                    slot: 0,
                    all: false,
                },
                17,
            ),
            (InventoryAction::DropFloating { all: false }, 18),
            (InventoryAction::QuickMoveOne(0), 19),
            (InventoryAction::OpenEngravingTable(IVec3::ZERO), 20),
            (
                InventoryAction::Enchant(crate::world::Enchantment::Efficiency),
                21,
            ),
            (InventoryAction::OpenAnvil(IVec3::ZERO), 22),
            (InventoryAction::Forge(crate::world::ItemName::default()), 23),
        ] {
            assert_eq!(variant_index(&action), expected, "{ENCODING_WARNING}");
        }
        for (mode, expected) in [(GameMode::Survival, 0), (GameMode::Creative, 1)] {
            assert_eq!(variant_index(&mode), expected, "{ENCODING_WARNING}");
        }
        for (cause, expected) in [
            (DeathCause::Fall, 0),
            (DeathCause::Void, 1),
            (DeathCause::Drowning, 2),
            (DeathCause::Fire, 3),
            (DeathCause::Lava, 4),
            (DeathCause::Starvation, 5),
            (DeathCause::Mob(MobKind::Fox), 6),
            (DeathCause::ThrownOff(MobKind::Fox), 7),
        ] {
            assert_eq!(variant_index(&cause), expected, "{ENCODING_WARNING}");
        }
        assert_eq!(variant_index(&MobKind::Fox), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Sheep), 1, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Cow), 2, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Pig), 3, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Chicken), 4, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Rabbit), 5, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Wolf), 6, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Horse), 7, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Goat), 8, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Cat), 9, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Gnawer), 10, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Bramble), 11, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Roller), 12, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Stumpling), 13, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Drowner), 14, "{ENCODING_WARNING}");
        assert_eq!(
            variant_index(&MobKind::Cinderling),
            15,
            "{ENCODING_WARNING}"
        );
        assert_eq!(variant_index(&MobKind::Ember), 16, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::GustOwl), 17, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Arrow), 18, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Farmer), 19, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobKind::Smith), 20, "{ENCODING_WARNING}");
        assert_eq!(
            variant_index(&MobKind::Cartographer),
            21,
            "{ENCODING_WARNING}"
        );
        assert_eq!(
            variant_index(&MobKind::EmberMother),
            22,
            "{ENCODING_WARNING}"
        );
        assert_eq!(variant_index(&MobKind::Boat), 23, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobTarget::None), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobTarget::Position(Vec3::ZERO)), 1);
        assert_eq!(variant_index(&MobTarget::Player(0)), 2);
        assert_eq!(variant_index(&MobAction::Walk), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Flee), 1, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Idle), 2, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Graze), 3, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Chase), 4, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Attack), 5, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Burrow), 6, "{ENCODING_WARNING}");
        for (kind, expected) in [
            (WeatherKind::Clear, 0),
            (WeatherKind::Rain, 1),
            (WeatherKind::Thunder, 2),
        ] {
            assert_eq!(variant_index(&kind), expected, "{ENCODING_WARNING}");
        }
        for (color, expected) in [(PortalColor::Blue, 0), (PortalColor::Orange, 1)] {
            assert_eq!(variant_index(&color), expected, "{ENCODING_WARNING}");
        }
        for (face, expected) in [
            (FaceDirection::PlusX, 0),
            (FaceDirection::MinusX, 1),
            (FaceDirection::PlusY, 2),
            (FaceDirection::MinusY, 3),
            (FaceDirection::PlusZ, 4),
            (FaceDirection::MinusZ, 5),
        ] {
            assert_eq!(variant_index(&face), expected, "{ENCODING_WARNING}");
        }
        for (removal, expected) in [
            (ItemRemoval::Gone, 0),
            (ItemRemoval::PickedUp(0), 1),
            (ItemRemoval::Merged(0), 2),
        ] {
            assert_eq!(variant_index(&removal), expected, "{ENCODING_WARNING}");
        }
    }

    #[test]
    fn the_drops_and_the_move_of_one_item_round_trip_on_the_wire() {
        use bincode::Options;
        for action in [
            InventoryAction::DropSlot {
                slot: 74,
                all: true,
            },
            InventoryAction::DropFloating { all: true },
            InventoryAction::QuickMoveOne(74),
        ] {
            let bytes = bincode::options().serialize(&action).unwrap();
            let back: InventoryAction = bincode::options().deserialize(&bytes).unwrap();
            assert_eq!(back, action);
        }
    }

    #[test]
    fn a_drag_s_slots_round_trip_on_the_wire() {
        use bincode::Options;
        let drag = InventoryAction::Drag {
            slots: 1 << 127 | 1 << 74 | 1,
            one_each: true,
        };
        let bytes = bincode::options().serialize(&drag).unwrap();
        let back: InventoryAction = bincode::options().deserialize(&bytes).unwrap();
        assert_eq!(back, drag);
    }

    #[test]
    fn a_portal_shot_reaches_a_client_as_it_was_sent() {
        let shot = ServerToClientMessage::PortalShot {
            shooter: 3,
            from: Vec3::new(1.0, 2.5, -3.0),
            to: Vec3::new(20.25, 11.0, 7.5),
            color: PortalColor::Orange,
            placed: true,
        };
        let payload = crate::game_message_to_payload(shot.clone());
        let received = crate::payload_to_game_message::<ServerToClientMessage>(&payload).unwrap();
        assert_eq!(format!("{received:?}"), format!("{shot:?}"));
    }
}
