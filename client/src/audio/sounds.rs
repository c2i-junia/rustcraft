//! The sounds of the game and the files they are played from. The files are made by
//! `scripts/generate_sounds.py` and, for the later features,
//! `scripts/generate_feature_sounds.py`, in `data/sounds/`.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use shared::players::armour::ArmourMaterial;
use shared::world::{BlockId, MobKind};

use super::settings::Channel;

/// What a block sounds like when it is walked on, mined, broken or placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundMaterial {
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Sand,
    Snow,
    Glass,
}

impl SoundMaterial {
    pub const ALL: [SoundMaterial; 8] = [
        SoundMaterial::Grass,
        SoundMaterial::Dirt,
        SoundMaterial::Stone,
        SoundMaterial::Wood,
        SoundMaterial::Leaves,
        SoundMaterial::Sand,
        SoundMaterial::Snow,
        SoundMaterial::Glass,
    ];

    /// The material of `block`; `None` for the liquids and fire, which make no block sound.
    pub fn of(block: BlockId) -> Option<Self> {
        Some(match block {
            BlockId::Grass
            | BlockId::TallGrass
            | BlockId::Fern
            | BlockId::Vine
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::Cactus
            | BlockId::Thatch
            | BlockId::Reed
            | BlockId::Heather
            | BlockId::Mushroom
            | BlockId::OakSapling
            | BlockId::SpruceSapling
            | BlockId::JungleSapling
            | BlockId::WillowSapling
            | BlockId::AcaciaSapling
            | BlockId::AppleSapling
            | BlockId::Wool
            | BlockId::BerryBush
            | BlockId::RipeBerryBush
            | BlockId::RyeSprout
            | BlockId::YoungRye
            | BlockId::RipeRye
            | BlockId::CarrotSprout
            | BlockId::YoungCarrots
            | BlockId::TallCarrots
            | BlockId::RipeCarrots
            | BlockId::PotatoSprout
            | BlockId::YoungPotatoes
            | BlockId::TallPotatoes
            | BlockId::RipePotatoes
            | BlockId::Bedroll
            | BlockId::Kelp
            | BlockId::Seagrass
            | BlockId::Moss
            | BlockId::CaveVine => Self::Grass,
            BlockId::Dirt | BlockId::Clay | BlockId::RammedEarth | BlockId::TilledSoil => {
                Self::Dirt
            }
            BlockId::HewnStoneStairs
            | BlockId::BrickStairs
            | BlockId::HangingLantern
            | BlockId::RainCistern
            | BlockId::FullRainCistern
            | BlockId::Stone
            | BlockId::Cobblestone
            | BlockId::Bedrock
            | BlockId::Debug
            | BlockId::Furnace
            | BlockId::LitFurnace
            | BlockId::CoalOre
            | BlockId::IronOre
            | BlockId::GoldOre
            | BlockId::DiamondOre
            | BlockId::IronBlock
            | BlockId::GoldBlock
            | BlockId::DiamondBlock
            | BlockId::HewnStone
            | BlockId::Limestone
            | BlockId::Slate
            | BlockId::Gloomrock
            | BlockId::CobbledGloomrock
            | BlockId::BrickWall
            | BlockId::CoalBlock
            | BlockId::CopperOre
            | BlockId::TinOre
            | BlockId::CopperBlock
            | BlockId::TinBlock
            | BlockId::BronzeBlock
            | BlockId::HewnStoneSlab
            | BlockId::BrickSlab
            | BlockId::Lantern
            | BlockId::Obsidian
            | BlockId::EmberHeart
            | BlockId::EngravingTable
            | BlockId::Anvil
            | BlockId::AzureReef
            | BlockId::RoseReef
            | BlockId::AmberReef
            | BlockId::Geodestone
            | BlockId::Basalt
            | BlockId::PolishedBasalt
            | BlockId::Magma
            | BlockId::Alembic => Self::Stone,
            BlockId::OakLog
            | BlockId::OakPlanks
            | BlockId::SpruceLog
            | BlockId::SprucePlanks
            | BlockId::JungleLog
            | BlockId::JunglePlanks
            | BlockId::CraftingTable
            | BlockId::Chest
            | BlockId::Torch
            | BlockId::Cocoa
            | BlockId::WillowLog
            | BlockId::WillowPlanks
            | BlockId::AcaciaLog
            | BlockId::AcaciaPlanks
            | BlockId::OakSlab
            | BlockId::OakStairs
            | BlockId::Door
            | BlockId::DoorTop
            | BlockId::OpenDoor
            | BlockId::OpenDoorTop
            | BlockId::Ladder
            | BlockId::Fence
            | BlockId::Barrel
            | BlockId::Campfire
            | BlockId::UnlitCampfire
            | BlockId::Sign
            | BlockId::MapFrame => Self::Wood,
            BlockId::OakLeaves
            | BlockId::SpruceLeaves
            | BlockId::JungleLeaves
            | BlockId::WillowLeaves
            | BlockId::AcaciaLeaves
            | BlockId::AppleLeaves => Self::Leaves,
            BlockId::Sand | BlockId::Gravel => Self::Sand,
            BlockId::Snow => Self::Snow,
            BlockId::Glass
            | BlockId::Ice
            | BlockId::GlassPane
            | BlockId::Glowcrystal
            | BlockId::CrystalCluster => Self::Glass,
            BlockId::Water | BlockId::Lava | BlockId::Fire => return None,
            // as the block they are made of
            shared::new_palette_blocks!() => return Self::of(block.palette()?.base),
            shared::new_decor_blocks!() => return Self::of(block.decor()?.base),
        })
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Dirt => "dirt",
            Self::Stone => "stone",
            Self::Wood => "wood",
            Self::Leaves => "leaves",
            Self::Sand => "sand",
            Self::Snow => "snow",
            Self::Glass => "glass",
        }
    }

    /// The material whose footsteps are heard on this one: nobody walks on leaves or glass
    /// often enough for them to have their own.
    fn footsteps(self) -> Self {
        match self {
            Self::Leaves => Self::Grass,
            Self::Glass => Self::Stone,
            other => other,
        }
    }
}

/// What a creature says (`audio::creatures`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CreatureCall {
    /// Heard now and then, from an animal or a hostile not hunting; from a projectile as it is sent.
    Cry,
    /// A hostile hunts a player.
    Approach,
    /// A hostile strikes, winds up or throws.
    Attack,
    /// The creature is hurt.
    Hurt,
}

impl CreatureCall {
    fn file_name(self) -> &'static str {
        match self {
            CreatureCall::Cry => "cry",
            CreatureCall::Approach => "approach",
            CreatureCall::Attack => "attack",
            CreatureCall::Hurt => "hurt",
        }
    }

    /// What creatures of `kind` say: every one cries and is hurt, a hostile approaches and
    /// attacks too, and a projectile, never struck, only flies, and a
    /// villager speaks and is hurt as an animal is.
    pub fn of(kind: MobKind) -> &'static [CreatureCall] {
        if matches!(
            kind,
            MobKind::Arrow | MobKind::Bobber | MobKind::ThrownPotion
        ) || kind.is_vehicle()
        {
            // `creatures::creature_calls` gives the arrow other sounds, `mob::sync` the float
            // its own; a boat has no voice
            &[]
        } else if kind.is_villager() {
            &[CreatureCall::Cry, CreatureCall::Hurt]
        } else if kind.is_projectile() {
            &[CreatureCall::Cry]
        } else if kind.is_hostile() {
            &[
                CreatureCall::Cry,
                CreatureCall::Approach,
                CreatureCall::Attack,
                CreatureCall::Hurt,
            ]
        } else {
            &[CreatureCall::Cry, CreatureCall::Hurt]
        }
    }
}

/// The folder of the sounds of `kind` in `sounds/mob/`.
fn creature_folder(kind: MobKind) -> &'static str {
    match kind {
        MobKind::Fox => "fox",
        MobKind::Sheep => "sheep",
        MobKind::Cow => "cow",
        MobKind::Pig => "pig",
        MobKind::Chicken => "chicken",
        MobKind::Rabbit => "rabbit",
        MobKind::Wolf => "wolf",
        MobKind::Horse => "horse",
        MobKind::Goat => "goat",
        MobKind::Cat => "cat",
        MobKind::Gnawer => "gnawer",
        MobKind::Bramble => "bramble",
        MobKind::Roller => "roller",
        MobKind::Stumpling => "stumpling",
        MobKind::Drowner => "drowner",
        MobKind::Cinderling => "cinderling",
        MobKind::EmberMother => "ember_mother",
        MobKind::Ember => "ember",
        MobKind::GustOwl => "gust_owl",
        MobKind::Arrow => "arrow",
        MobKind::Farmer => "farmer",
        MobKind::Smith => "smith",
        MobKind::Cartographer => "cartographer",
        MobKind::Boat => "boat",
        MobKind::Bobber => "bobber",
        MobKind::Glintfin => "glintfin",
        MobKind::Shoalback => "shoalback",
        MobKind::Mossback => "mossback",
        MobKind::Shardling => "shardling",
        MobKind::Slagcrawler => "slagcrawler",
        MobKind::Donkey => "donkey",
        MobKind::Mule => "mule",
        MobKind::ThrownPotion => "thrown_potion",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    Step(SoundMaterial),
    /// One hit of a block being mined.
    Dig(SoundMaterial),
    Break(SoundMaterial),
    Place(SoundMaterial),
    /// The local player lands after a fall.
    Land,
    /// The local player is hurt.
    Hurt,
    /// A button of the interface is pressed.
    Click,
    /// The local player picks up an item lying in the world.
    Pickup,
    /// A bucket takes a water source, or pours one.
    Splash,
    /// A stack goes onto the cursor in the inventory.
    ItemPick,
    /// Items leave the cursor into slots of the inventory.
    ItemPlace,
    /// A craft took the result of the crafting grid.
    Craft,
    /// An action the player made was refused.
    Refused,
    /// A lightning strike close by: a tearing crack, a boom and its roll.
    Thunder,
    /// A lightning strike far off: a long low rumble.
    DistantThunder,
    /// The rain heard out in the open, a seamless loop.
    Rain,
    /// The same rain heard from under a roof, a seamless loop.
    RainIndoors,
    /// The wind, a seamless loop.
    Wind,
    /// A portal opens: its twin was placed, or it was placed with its twin there.
    PortalOpen,
    /// An arrow let fly.
    BowLoose,
    /// The local player goes through a portal.
    PortalPass,
    /// A creature of a kind says something: each kind has a voice of its own.
    Creature(MobKind, CreatureCall),
    /// A piece of armour of the material is put on.
    ArmourOn(ArmourMaterial),
    /// A piece of armour of the material is taken off.
    ArmourOff(ArmourMaterial),
    /// A piece of armour worn out breaks.
    ArmourBreak,
    /// A bow starts to be drawn: the creak of its stave, as long as a full draw.
    BowDraw,
    /// An arrow sticks in a block.
    ArrowHitBlock,
    /// An arrow strikes a creature.
    ArrowHitCreature,
    /// An arrow strikes a player.
    ArrowHitPlayer,
    /// A stuck arrow is picked up.
    ArrowPickup,
    /// The local player picks up an orb of experience: three pitches.
    Orb,
    /// The local player goes up a level.
    LevelUp,
    /// The engraving table opens.
    EngravingOpen,
    /// An enchantment is engraved.
    Enchant,
    /// An enchantment clicked that the player cannot have.
    EnchantRefused,
    /// The anvil opens: the hammer laid on it.
    AnvilOpen,
    /// The anvil works an item: the hammer strikes.
    AnvilUse,
    /// A trade with a villager opens.
    TradeOpen,
    /// An offer is taken: coins.
    TradeDone,
    /// An offer clicked that cannot be taken.
    TradeRefused,
    /// An animal starts to want a mate: hearts rise.
    Love,
    /// A young animal is born.
    Birth,
    /// An animal is tamed.
    Tamed,
    /// A mob falls into water.
    MobSplash,
    /// The local player reaches a deeper tier: a low sting.
    DepthReached,
    /// An achievement is earned.
    Achievement,
    /// The bar of a boss appears.
    BossBar,
    /// The boss is beaten.
    Victory,
    /// The credits start.
    Credits,
    /// A skin dropped on the skins screen is imported.
    SkinImported,
    /// A critical blow lands: a bright crack.
    CriticalHit,
    /// A sword sweeps: the blade cutting the air.
    Sweep,
    /// A saddle is put on a horse.
    Saddle,
    /// A boat is rowed on the water: a stroke of its oars.
    BoatPaddle,
    /// A boat is struck: its hull knocks.
    BoatHit,
    /// A shield, or a Stumpling's bark, parries a blow or a shot: a dull wooden knock.
    ShieldBlock,
    /// A crossbow is loaded: its string cranked back to the latch.
    CrossbowLoad,
    /// A loaded crossbow shoots: the latch let go and the string's snap.
    CrossbowShot,
    /// A fishing rod casts its float: the line whistling out of the reel.
    FishingCast,
    /// A fish bites: the float plunges with a plop and a gurgle.
    FishingBite,
    /// The float drawn in: the reel winding and a drip of water.
    FishingReel,
    /// Horse armour is put on a horse: its plates clank.
    HorseArmour,
    /// A lead is tied or untied: the rope creaks.
    Lead,
    /// A chest is strapped on a donkey or a mule.
    PackChest,
    /// A wolf bites: a growl and the snap of its jaws.
    WolfBite,
    /// A player sits down on a chair: the seat knocks and its joints creak.
    Sit,
    /// An item is put in a frame or turned in it: a light wooden click.
    FrameTurn,
    /// An alembic opens: the liquid in its flask simmers.
    AlembicOpen,
    /// The local player drinks a potion: gulps.
    PotionDrink,
    /// A splash potion bursts: glass shatters and the liquid splashes.
    PotionShatter,
    /// A glass vial is filled at the water.
    VialFill,
}

impl Sound {
    pub fn all() -> impl Iterator<Item = Sound> {
        SoundMaterial::ALL
            .into_iter()
            .flat_map(|material| {
                [
                    Sound::Step(material),
                    Sound::Dig(material),
                    Sound::Break(material),
                    Sound::Place(material),
                ]
            })
            .chain([
                Sound::Land,
                Sound::Hurt,
                Sound::Click,
                Sound::Pickup,
                Sound::Splash,
                Sound::ItemPick,
                Sound::ItemPlace,
                Sound::Craft,
                Sound::Refused,
                Sound::PortalOpen,
                Sound::PortalPass,
            ])
            .chain(Self::FEATURES)
            .chain(
                Self::ARMOUR_MATERIALS
                    .into_iter()
                    .flat_map(|material| [Sound::ArmourOn(material), Sound::ArmourOff(material)]),
            )
            .chain(Self::WEATHER)
            .chain(MobKind::ALL.into_iter().flat_map(|kind| {
                CreatureCall::of(kind)
                    .iter()
                    .map(move |&call| Sound::Creature(kind, call))
            }))
    }

    const ARMOUR_MATERIALS: [ArmourMaterial; 4] = [
        ArmourMaterial::Leather,
        ArmourMaterial::Bronze,
        ArmourMaterial::Iron,
        ArmourMaterial::Diamond,
    ];

    /// The sounds of the later features (`scripts/generate_feature_sounds.py`), but those of
    /// the armour put on and off and the voices of the villagers.
    pub const FEATURES: [Sound; 48] = [
        Sound::BowLoose,
        Sound::ArmourBreak,
        Sound::BowDraw,
        Sound::ArrowHitBlock,
        Sound::ArrowHitCreature,
        Sound::ArrowHitPlayer,
        Sound::ArrowPickup,
        Sound::Orb,
        Sound::LevelUp,
        Sound::EngravingOpen,
        Sound::Enchant,
        Sound::EnchantRefused,
        Sound::AnvilOpen,
        Sound::AnvilUse,
        Sound::TradeOpen,
        Sound::TradeDone,
        Sound::TradeRefused,
        Sound::Love,
        Sound::Birth,
        Sound::Tamed,
        Sound::MobSplash,
        Sound::DepthReached,
        Sound::Achievement,
        Sound::BossBar,
        Sound::Victory,
        Sound::Credits,
        Sound::SkinImported,
        Sound::CriticalHit,
        Sound::Sweep,
        Sound::Saddle,
        Sound::BoatPaddle,
        Sound::BoatHit,
        Sound::ShieldBlock,
        Sound::CrossbowLoad,
        Sound::CrossbowShot,
        Sound::FishingCast,
        Sound::FishingBite,
        Sound::FishingReel,
        Sound::HorseArmour,
        Sound::Lead,
        Sound::PackChest,
        Sound::WolfBite,
        Sound::Sit,
        Sound::FrameTurn,
        Sound::AlembicOpen,
        Sound::PotionDrink,
        Sound::PotionShatter,
        Sound::VialFill,
    ];

    /// The sounds of the weather, played by `audio::weather`.
    pub const WEATHER: [Sound; 5] = [
        Sound::Thunder,
        Sound::DistantThunder,
        Sound::Rain,
        Sound::RainIndoors,
        Sound::Wind,
    ];

    pub fn channel(self) -> Channel {
        match self {
            Sound::Click
            | Sound::ItemPick
            | Sound::ItemPlace
            | Sound::Craft
            | Sound::Refused
            | Sound::EngravingOpen
            | Sound::Enchant
            | Sound::EnchantRefused
            | Sound::AnvilOpen
            | Sound::AnvilUse
            | Sound::TradeOpen
            | Sound::TradeDone
            | Sound::TradeRefused
            | Sound::Achievement
            | Sound::SkinImported => Channel::Interface,
            // The stings of the progress, short pieces of music
            Sound::DepthReached | Sound::BossBar | Sound::Victory | Sound::Credits => {
                Channel::Music
            }
            Sound::Thunder
            | Sound::DistantThunder
            | Sound::Rain
            | Sound::RainIndoors
            | Sound::Wind => Channel::Weather,
            _ => Channel::Effects,
        }
    }

    /// The files of the sound, relative to the assets folder: one per variant, of which one
    /// is picked at random each time it plays.
    pub fn files(self) -> Vec<String> {
        let variants = |kind: &str, material: SoundMaterial, count: u32| -> Vec<String> {
            (1..=count)
                .map(|i| format!("sounds/{kind}/{}{i}.ogg", material.file_name()))
                .collect()
        };
        let one = |name: &str| vec![format!("sounds/{name}.ogg")];
        match self {
            Sound::Step(material) => variants("step", material.footsteps(), 4),
            Sound::Dig(material) => variants("dig", material, 3),
            Sound::Break(material) => variants("break", material, 3),
            Sound::Place(material) => vec![format!("sounds/place/{}.ogg", material.file_name())],
            Sound::Land => vec!["sounds/player/land.ogg".to_string()],
            Sound::Hurt => vec!["sounds/player/hurt.ogg".to_string()],
            Sound::Click => vec!["sounds/ui/click.ogg".to_string()],
            // The click, short and soft enough: `player/pickup.ogg` sounds passing through a
            // portal
            Sound::Pickup => vec!["sounds/ui/click.ogg".to_string()],
            Sound::Splash => vec!["sounds/liquid/splash.ogg".to_string()],
            Sound::ItemPick => vec!["sounds/ui/item_pick.ogg".to_string()],
            Sound::ItemPlace => vec!["sounds/ui/item_place.ogg".to_string()],
            Sound::Craft => vec!["sounds/ui/craft.ogg".to_string()],
            Sound::Refused => vec!["sounds/ui/refused.ogg".to_string()],
            Sound::Thunder => (1..=3)
                .map(|i| format!("sounds/weather/thunder_near{i}.ogg"))
                .collect(),
            Sound::DistantThunder => (1..=3)
                .map(|i| format!("sounds/weather/thunder_far{i}.ogg"))
                .collect(),
            Sound::Rain => vec!["sounds/weather/rain.ogg".to_string()],
            Sound::RainIndoors => vec!["sounds/weather/rain_indoor.ogg".to_string()],
            Sound::Wind => vec!["sounds/ambient/wind.ogg".to_string()],
            // No sound of their own: a knock and a glassy ring, and a short rising pop
            Sound::PortalOpen => vec!["sounds/place/glass.ogg".to_string()],
            Sound::BowLoose => one("bow/loose"),
            Sound::ArmourOn(material) => one(&format!("armour/{}_on", armour_name(material))),
            Sound::ArmourOff(material) => one(&format!("armour/{}_off", armour_name(material))),
            Sound::ArmourBreak => one("armour/break"),
            Sound::BowDraw => one("bow/draw"),
            Sound::ArrowHitBlock => one("bow/hit_block"),
            Sound::ArrowHitCreature => one("bow/hit_creature"),
            Sound::ArrowHitPlayer => one("bow/hit_player"),
            Sound::ArrowPickup => one("bow/pickup"),
            // Three pitches, a little apart
            Sound::Orb => (1..=3)
                .map(|i| format!("sounds/player/orb{i}.ogg"))
                .collect(),
            Sound::LevelUp => one("player/level_up"),
            Sound::EngravingOpen => one("ui/engraving_open"),
            Sound::Enchant => one("ui/enchant"),
            Sound::EnchantRefused => one("ui/enchant_refused"),
            Sound::AnvilOpen => one("ui/anvil_open"),
            Sound::AnvilUse => one("ui/anvil_use"),
            Sound::TradeOpen => one("ui/trade_open"),
            Sound::TradeDone => one("ui/trade_done"),
            Sound::TradeRefused => one("ui/trade_refused"),
            Sound::Love => one("mob/love"),
            Sound::Birth => one("mob/birth"),
            Sound::Tamed => one("mob/tamed"),
            Sound::MobSplash => one("liquid/mob_splash"),
            Sound::DepthReached => one("ui/depth"),
            Sound::Achievement => one("ui/achievement"),
            Sound::BossBar => one("ui/boss_bar"),
            Sound::Victory => one("ui/victory"),
            Sound::Credits => one("ui/credits"),
            Sound::SkinImported => one("ui/skin_import"),
            Sound::CriticalHit => one("combat/critical"),
            Sound::Sweep => one("combat/sweep"),
            Sound::Saddle => one("mob/saddle"),
            Sound::BoatPaddle => one("boat/paddle"),
            Sound::BoatHit => one("boat/hit"),
            Sound::ShieldBlock => one("combat/shield_block"),
            Sound::CrossbowLoad => one("combat/crossbow_load"),
            Sound::CrossbowShot => one("combat/crossbow_shot"),
            Sound::FishingCast => one("fishing/cast"),
            Sound::FishingBite => one("fishing/bite"),
            Sound::FishingReel => one("fishing/reel"),
            Sound::HorseArmour => one("mob/horse_armour"),
            Sound::Lead => one("mob/lead"),
            Sound::PackChest => one("mob/pack_chest"),
            Sound::WolfBite => one("mob/wolf_bite"),
            Sound::Sit => one("furniture/sit"),
            Sound::FrameTurn => one("furniture/frame"),
            Sound::AlembicOpen => one("alchemy/alembic"),
            Sound::PotionDrink => one("alchemy/drink"),
            Sound::PotionShatter => one("alchemy/shatter"),
            Sound::VialFill => one("alchemy/fill"),
            Sound::PortalPass => vec!["sounds/player/pickup.ogg".to_string()],
            Sound::Creature(kind, call) => {
                let folder = creature_folder(kind);
                let name = call.file_name();
                // Two cries, so a flock does not repeat itself; a projectile has one
                let variants = match call {
                    CreatureCall::Cry if !kind.is_projectile() => 2,
                    _ => 1,
                };
                if call == CreatureCall::Cry {
                    (1..=variants)
                        .map(|i| format!("sounds/mob/{folder}/{name}{i}.ogg"))
                        .collect()
                } else {
                    vec![format!("sounds/mob/{folder}/{name}.ogg")]
                }
            }
        }
    }
}

/// The name of `material` in the files of the armour sounds.
fn armour_name(material: ArmourMaterial) -> &'static str {
    match material {
        ArmourMaterial::Leather => "leather",
        ArmourMaterial::Bronze => "bronze",
        ArmourMaterial::Iron => "iron",
        ArmourMaterial::Diamond => "diamond",
    }
}

/// The loaded variants of every sound.
#[derive(Resource, Default)]
pub struct SoundLibrary(pub HashMap<Sound, Vec<Handle<AudioSource>>>);

pub fn load_sound_library(mut commands: Commands, asset_server: Res<AssetServer>) {
    let library = Sound::all()
        .map(|sound| {
            let handles = sound
                .files()
                .into_iter()
                .map(|file| asset_server.load(file))
                .collect();
            (sound, handles)
        })
        .collect();
    commands.insert_resource(SoundLibrary(library));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_sound_has_a_file() {
        for sound in Sound::all() {
            assert!(!sound.files().is_empty(), "{sound:?}");
        }
    }

    #[test]
    fn every_file_of_every_sound_is_in_the_data_folder() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        for sound in Sound::all() {
            for file in sound.files() {
                assert!(data.join(&file).is_file(), "{sound:?}: missing {file}");
            }
        }
    }

    const INVENTORY_SOUNDS: [Sound; 4] = [
        Sound::ItemPick,
        Sound::ItemPlace,
        Sound::Craft,
        Sound::Refused,
    ];

    #[test]
    fn the_inventory_sounds_play_on_the_interface_channel() {
        for sound in INVENTORY_SOUNDS {
            assert!(Sound::all().any(|known| known == sound), "{sound:?}");
            assert_eq!(sound.channel(), Channel::Interface, "{sound:?}");
        }
    }

    /// The length of the file, and its loudest sample from 0 to 1.
    fn length_and_peak(file: &str) -> (std::time::Duration, f32) {
        use bevy::audio::{Decodable, Source};

        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let bytes = std::fs::read(data.join(file)).unwrap();
        let decoder = AudioSource {
            bytes: bytes.into(),
        }
        .decoder();
        let rate = decoder.sample_rate().get() as f32;
        let channels = decoder.channels().get() as f32;
        let samples: Vec<f32> = decoder.collect();
        let length = samples.len() as f32 / channels / rate;
        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        (std::time::Duration::from_secs_f32(length), peak)
    }

    #[test]
    fn the_inventory_sounds_are_short_and_neither_clipped_nor_silent() {
        for sound in INVENTORY_SOUNDS {
            let longest = match sound {
                Sound::ItemPick | Sound::ItemPlace => 120,
                _ => 300,
            };
            for file in sound.files() {
                let (length, peak) = length_and_peak(&file);
                assert!(
                    length < std::time::Duration::from_millis(longest),
                    "{file}: {length:?}"
                );
                // Under 0 dBFS, over -20 dBFS
                assert!(peak < 1.0 && peak > 0.1, "{file}: peak {peak}");
            }
        }
    }

    #[test]
    fn the_rain_the_wind_and_the_thunder_play_on_the_weather_channel() {
        for sound in Sound::WEATHER {
            assert_eq!(sound.channel(), Channel::Weather, "{sound:?}");
            assert!(Sound::all().any(|other| other == sound), "{sound:?}");
        }
        assert_eq!(Sound::Thunder.files().len(), 3);
        assert_eq!(Sound::DistantThunder.files().len(), 3);
        assert_eq!(Sound::Splash.channel(), Channel::Effects);
    }

    #[test]
    fn every_creature_has_a_cry_and_every_hostile_its_approach_and_attack() {
        for kind in MobKind::ALL.into_iter().filter(|kind| !kind.is_villager()) {
            let calls = CreatureCall::of(kind);
            assert!(
                calls.contains(&CreatureCall::Cry)
                    || matches!(
                        kind,
                        MobKind::Arrow | MobKind::Bobber | MobKind::ThrownPotion
                    )
                    || kind.is_vehicle(),
                "{kind:?}"
            );
            assert_eq!(
                calls.contains(&CreatureCall::Approach),
                kind.is_hostile(),
                "{kind:?}"
            );
            assert_eq!(
                calls.contains(&CreatureCall::Attack),
                kind.is_hostile(),
                "{kind:?}"
            );
            assert_eq!(
                calls.contains(&CreatureCall::Hurt),
                // A struck boat knocks (`Sound::BoatHit`)
                !kind.is_projectile() && !kind.is_vehicle(),
                "{kind:?}"
            );
            for &call in calls {
                assert_eq!(Sound::Creature(kind, call).channel(), Channel::Effects);
            }
        }
    }

    #[test]
    fn no_two_creatures_share_a_voice() {
        let mut files = std::collections::HashSet::new();
        for kind in MobKind::ALL {
            for &call in CreatureCall::of(kind) {
                for file in Sound::Creature(kind, call).files() {
                    assert!(files.insert(file.clone()), "{file}");
                }
            }
        }
    }

    #[test]
    fn the_creature_sounds_are_neither_clipped_nor_silent() {
        for kind in MobKind::ALL {
            for &call in CreatureCall::of(kind) {
                for file in Sound::Creature(kind, call).files() {
                    let (length, peak) = length_and_peak(&file);
                    assert!(length < std::time::Duration::from_secs(3), "{file}");
                    assert!(peak < 1.0 && peak > 0.1, "{file}: peak {peak}");
                }
            }
        }
    }

    /// The sounds of the later features, and the voices of the villagers.
    fn feature_sounds() -> Vec<Sound> {
        let villagers = MobKind::ALL
            .into_iter()
            .filter(|kind| kind.is_villager())
            .flat_map(|kind| {
                CreatureCall::of(kind)
                    .iter()
                    .map(move |&call| Sound::Creature(kind, call))
            });
        Sound::FEATURES
            .into_iter()
            .chain(
                Sound::ARMOUR_MATERIALS
                    .into_iter()
                    .flat_map(|m| [Sound::ArmourOn(m), Sound::ArmourOff(m)]),
            )
            .chain(villagers)
            .collect()
    }

    #[test]
    fn the_feature_sounds_are_known_short_and_mixed_as_the_others() {
        for sound in feature_sounds() {
            assert!(Sound::all().any(|known| known == sound), "{sound:?}");
            for file in sound.files() {
                let (length, peak) = length_and_peak(&file);
                // The stings of the music ring longer
                let longest = match sound.channel() {
                    Channel::Music => 3000,
                    _ => 1500,
                };
                assert!(
                    length < std::time::Duration::from_millis(longest),
                    "{file}: {length:?}"
                );
                // Under -1 dBFS, over -20 dBFS, as the others
                assert!(peak < 0.9 && peak > 0.1, "{file}: peak {peak}");
            }
        }
    }

    #[test]
    fn every_villager_has_a_voice_of_its_own() {
        for kind in MobKind::ALL.into_iter().filter(|kind| kind.is_villager()) {
            assert_eq!(
                CreatureCall::of(kind),
                [CreatureCall::Cry, CreatureCall::Hurt]
            );
        }
    }

    #[test]
    fn the_fern_sounds_like_grass() {
        assert_eq!(SoundMaterial::of(BlockId::Fern), Some(SoundMaterial::Grass));
    }

    #[test]
    fn the_vine_sounds_like_grass() {
        assert_eq!(SoundMaterial::of(BlockId::Vine), Some(SoundMaterial::Grass));
    }

    #[test]
    fn the_lit_furnace_sounds_like_stone() {
        assert_eq!(
            SoundMaterial::of(BlockId::LitFurnace),
            Some(SoundMaterial::Stone)
        );
    }

    #[test]
    fn diamond_ore_and_the_storage_blocks_sound_like_stone() {
        // there is no metal sound
        for block in [
            BlockId::DiamondOre,
            BlockId::IronBlock,
            BlockId::GoldBlock,
            BlockId::DiamondBlock,
        ] {
            assert_eq!(
                SoundMaterial::of(block),
                Some(SoundMaterial::Stone),
                "{block:?}"
            );
        }
    }

    #[test]
    fn the_cocoa_sounds_like_wood() {
        assert_eq!(SoundMaterial::of(BlockId::Cocoa), Some(SoundMaterial::Wood));
    }

    #[test]
    fn jungle_wood_sounds_like_wood_and_its_leaves_like_leaves() {
        assert_eq!(
            SoundMaterial::of(BlockId::JungleLog),
            Some(SoundMaterial::Wood)
        );
        assert_eq!(
            SoundMaterial::of(BlockId::JunglePlanks),
            Some(SoundMaterial::Wood)
        );
        assert_eq!(
            SoundMaterial::of(BlockId::JungleLeaves),
            Some(SoundMaterial::Leaves)
        );
    }
}
