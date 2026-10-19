//! The minimap in the top-right corner of the game: the loaded world seen from above, north up,
//! around an arrow that turns with the camera, with the coordinates of the player under it.
//! `M` shows or hides it, as the Interface tab of the settings does, and `=` and `-` zoom.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use shared::GameFolderPaths;

use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::player::CurrentPlayerMarker;
use crate::state::{GameSet, GameState, KeyMap};
use crate::ui::hud::corner::TopRightColumn;
use crate::ui::hud::debug::debug_font;
use crate::ui::hud::debug::in_gameplay;
use crate::ui::settings::{write_interface_settings, InterfaceSettings, MinimapChoice};
use crate::ui::theme::{ACCENT, BEVEL_WIDTH, PANEL, SPACE_XS, TEXT};
use crate::ui::widgets::raised_bevel;
use crate::world::{BlockChanged, ChunkLoadEvent, ClientWorldMap};

pub mod colors;
pub mod compose;
#[cfg(test)]
mod goldens;
mod markers;
pub mod surface;

use colors::{compute_minimap_colors, MinimapColors};
use compose::{compose, MAP_SIZE, UNLOADED};
use markers::{place_minimap_dots, point_minimap_arrow, spawn_arrow, write_minimap_coords};
use surface::{MinimapSurface, COLUMNS_PER_FRAME};

/// The blocks a pixel of the map shows a column of, from the nearest to the farthest view.
pub const ZOOMS: [i32; 3] = [1, 2, 4];

/// The frame of the minimap, hidden with it.
#[derive(Component)]
pub struct MinimapRoot;

/// The square the map, its arrow and its dots are drawn in.
#[derive(Component)]
pub struct MinimapView;

/// The coordinates of the player, under the map.
#[derive(Component)]
pub struct MinimapCoords;

/// The image of the map, and what it was last drawn of.
#[derive(Resource)]
pub struct MinimapImage {
    pub handle: Handle<Image>,
    /// The index in `ZOOMS` of the zoom of the map.
    pub zoom: usize,
    /// The centre, the zoom and the revision of the surface it shows.
    drawn: Option<(IVec2, i32, u64)>,
}

impl MinimapImage {
    /// The blocks a pixel shows a column of.
    pub fn blocks_per_pixel(&self) -> i32 {
        ZOOMS[self.zoom]
    }
}

pub fn minimap_plugin(app: &mut App) {
    app.add_systems(
        OnEnter(GameState::Game),
        (compute_minimap_colors, start_minimap_surface),
    )
    .add_systems(
        Update,
        (toggle_minimap, zoom_minimap)
            .after(GameSet::UiMode)
            .run_if(in_state(GameState::Game).and_then(in_gameplay)),
    )
    .add_systems(
        Update,
        (
            show_minimap,
            follow_the_world,
            draw_minimap,
            point_minimap_arrow,
            place_minimap_dots,
            write_minimap_coords,
        )
            .chain()
            .after(GameSet::Camera)
            .after(toggle_minimap)
            .after(zoom_minimap)
            .run_if(in_state(GameState::Game)),
    );
}

/// The surface of the chunks the client holds as the game starts, all to scan.
fn start_minimap_surface(world_map: Res<ClientWorldMap>, mut commands: Commands) {
    commands.insert_resource(MinimapSurface::of(&world_map.blocks));
}

/// Spawns the minimap at the top of `TopRightColumn`, shown by `show_minimap`.
pub fn spawn_minimap(
    mut commands: Commands,
    column: Single<Entity, With<TopRightColumn>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: MAP_SIZE,
            height: MAP_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[UNLOADED[0], UNLOADED[1], UNLOADED[2], 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    commands.insert_resource(MinimapImage {
        handle: handle.clone(),
        zoom: 0,
        drawn: None,
    });

    let font = debug_font(&assets);
    let root = commands
        .spawn((
            MinimapRoot,
            Name::new("Minimap"),
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_XS),
                padding: UiRect::all(px(SPACE_XS)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(*column),
        ))
        .id();
    let view = commands
        .spawn((
            MinimapView,
            ImageNode::new(handle),
            Node {
                width: px(MAP_SIZE),
                height: px(MAP_SIZE),
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Text::new("N"),
        font.clone(),
        TextColor(ACCENT),
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: Val::Percent(50.),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50., 0.)),
        ChildOf(view),
    ));
    spawn_arrow(&mut commands, &mut images, view);
    commands.spawn((
        MinimapCoords,
        Text::default(),
        font,
        TextColor(TEXT),
        ChildOf(root),
    ));
}

/// Shows the minimap as the settings ask. Hidden under the menus, by `hide_hud_under_menus`, it
/// keeps its room, so that the FPS counter under it does not move.
fn show_minimap(settings: Res<InterfaceSettings>, mut roots: Query<&mut Node, With<MinimapRoot>>) {
    let display = match settings.minimap {
        MinimapChoice::On => Display::Flex,
        MinimapChoice::Off => Display::None,
    };
    for mut node in &mut roots {
        if node.display != display {
            node.display = display;
        }
    }
}

/// Shows or hides the minimap on its key, and saves it.
fn toggle_minimap(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    if !is_action_just_pressed(GameAction::ToggleMinimap, &keyboard_input, &key_map) {
        return;
    }
    settings.minimap = match settings.minimap {
        MinimapChoice::On => MinimapChoice::Off,
        MinimapChoice::Off => MinimapChoice::On,
    };
    write_interface_settings(&settings, &paths);
}

/// Zooms the map in and out on their keys, within `ZOOMS`.
fn zoom_minimap(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    image: Option<ResMut<MinimapImage>>,
) {
    let Some(mut image) = image else {
        return;
    };
    if is_action_just_pressed(GameAction::MinimapZoomIn, &keyboard_input, &key_map) {
        image.zoom = image.zoom.saturating_sub(1);
    }
    if is_action_just_pressed(GameAction::MinimapZoomOut, &keyboard_input, &key_map) {
        image.zoom = (image.zoom + 1).min(ZOOMS.len() - 1);
    }
}

/// The column of the world a position is in, as (x, z).
pub fn column_of(position: Vec3) -> IVec2 {
    position.xz().floor().as_ivec2()
}

/// Keeps the surface up to date with the chunks and the blocks of the frame, scanning the
/// columns nearest to the player first.
fn follow_the_world(
    mut chunk_loads: MessageReader<ChunkLoadEvent>,
    mut block_changes: MessageReader<BlockChanged>,
    world_map: Res<ClientWorldMap>,
    surface: Option<ResMut<MinimapSurface>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    let Some(mut surface) = surface else {
        return;
    };
    for change in chunk_loads.read() {
        match *change {
            ChunkLoadEvent::Loaded(chunk) => surface.chunk_loaded(chunk),
            ChunkLoadEvent::Unloaded(chunk) => surface.chunk_unloaded(chunk),
        }
    }
    for change in block_changes.read() {
        surface.block_changed(&world_map.blocks, change.position);
    }
    if surface.has_dirty() {
        let near = player
            .single()
            .map(|transform| column_of(transform.translation).div_euclid(IVec2::splat(16)))
            .unwrap_or_default();
        surface.scan_dirty(&world_map.blocks, near, COLUMNS_PER_FRAME);
    }
}

/// Draws the map again when the player moved to another column, zoomed, or the surface changed,
/// while it is shown.
fn draw_minimap(
    settings: Res<InterfaceSettings>,
    image: Option<ResMut<MinimapImage>>,
    surface: Option<Res<MinimapSurface>>,
    colors: Option<Res<MinimapColors>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(mut image), Some(surface), Some(colors), Ok(player)) =
        (image, surface, colors, player.single())
    else {
        return;
    };
    if settings.minimap == MinimapChoice::Off {
        return;
    }
    let state = (
        column_of(player.translation),
        image.blocks_per_pixel(),
        surface.revision,
    );
    if image.drawn == Some(state) {
        return;
    }
    let Some(mut target) = images.get_mut(&image.handle) else {
        return;
    };
    let Some(pixels) = target.data.as_mut() else {
        return;
    };
    compose(&surface, &colors, state.0, state.1, pixels);
    image.drawn = Some(state);
}
