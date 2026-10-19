//! The world map: the whole loaded world over the whole screen, north up. Its key (`M`) opens it
//! during gameplay and closes it, as Escape does. It follows the player until dragged; Space
//! centres it on them again, and the wheel zooms it around the cursor, as `=` and `-` do around
//! its centre.
//!
//! The world is drawn into one image at one block a pixel, and the UI scales and moves that
//! image: dragging and zooming never draw it again. The arrow and the dots are children of the
//! image, placed in percent of it, so that they move with it and keep their size.

use std::time::Duration;

use accesskit::Role;
use bevy::a11y::AccessibilityNode;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use shared::players::Player;
use shared::CHUNK_SIZE;

use super::colors::MinimapColors;
use super::compose::{compose_world, UNLOADED};
use super::markers::{arrow_image, MinimapArrow, DOT_SIZE};
use super::surface::MinimapSurface;
use super::MinimapCoords;
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::mob::MobRoot;
use crate::player::CurrentPlayerMarker;
use crate::state::{GameState, KeyMap};
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::debug::debug_font;
use crate::ui::hud::{UIMode, UiDialog};
use crate::ui::theme::{
    ACCENT, BEVEL_LO, BEVEL_WIDTH, DANGER, FONT_L, PANEL, SLOT, SPACE_L, SPACE_XS, TEXT,
};
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::raised_bevel;
use crate::ui::widgets::scroll::scroll_steps;

/// The title of the world map.
pub const WORLD_MAP_TITLE: &str = "Map";

/// The logical px a block takes on the world map, from the farthest view to the nearest.
pub const SCALES: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

/// The index in `SCALES` the world map opens at, until the player zooms.
const FIRST_SCALE: usize = 1;

/// How often the open map is drawn again at most while the world comes in around the player.
const REDRAW_EVERY: Duration = Duration::from_millis(250);

/// The key hints of the world map.
pub const WORLD_MAP_HINTS: [Hint; 4] = [
    Hint {
        keys: "Drag",
        action: "Move",
    },
    Hint {
        keys: "Wheel",
        action: "Zoom",
    },
    Hint {
        keys: "Space",
        action: "Centre",
    },
    Hint {
        keys: "Esc",
        action: "Close",
    },
];

/// The root of the world map, spawned when it opens and despawned when it closes.
#[derive(Component)]
pub struct WorldMapRoot;

/// The part of the screen the map shows the world in, above the hint bar.
#[derive(Component)]
pub struct WorldMapView;

/// The image of the world, moved and scaled in the view.
#[derive(Component)]
pub struct WorldMapPicture;

/// The arrow of the player on the world map.
#[derive(Component)]
pub struct WorldMapArrow;

/// The dot of a player or of a mob on the world map.
#[derive(Component)]
pub struct WorldMapDot {
    pub target: Entity,
}

/// The image of the world map, what it was last drawn of, and how the player looks at it.
#[derive(Resource)]
pub struct WorldMap {
    pub handle: Handle<Image>,
    /// The image of the arrow of the player, and its size in logical px.
    arrow: (Handle<Image>, Vec2),
    /// The index in `SCALES` of the zoom, kept from one opening to the next.
    pub scale: usize,
    /// The point (x, z) of the world at the centre of the view, or `None` while it follows the
    /// player.
    pub focus: Option<Vec2>,
    /// The column (x, z) the top-left pixel of the image shows.
    pub corner: IVec2,
    /// The size of the image, in blocks.
    pub size: UVec2,
    /// The revision of the surface drawn, and when.
    drawn: Option<(u64, Duration)>,
}

impl WorldMap {
    /// The logical px a block takes.
    pub fn block_px(&self) -> f32 {
        SCALES[self.scale]
    }
}

/// Whether the world map is open.
pub fn world_map_open(roots: Query<(), With<WorldMapRoot>>) -> bool {
    !roots.is_empty()
}

/// An empty image for the world map, drawn when it opens.
pub fn start_world_map(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[UNLOADED[0], UNLOADED[1], UNLOADED[2], 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    commands.insert_resource(WorldMap {
        handle,
        arrow: arrow_image(&mut images),
        scale: FIRST_SCALE,
        focus: None,
        corner: IVec2::ZERO,
        size: UVec2::ZERO,
        drawn: None,
    });
}

/// Opens the world map on its key during gameplay, centred on the player, and closes it on its
/// key or on Escape. The pause menu leaves Escape alone while a dialog is open.
pub fn toggle_world_map(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    roots: Query<Entity, With<WorldMapRoot>>,
    map: Option<ResMut<WorldMap>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Some(mut map) = map else {
        return;
    };
    let key = is_action_just_pressed(GameAction::OpenMap, &keyboard_input, &key_map);
    if let Ok(root) = roots.single() {
        if key || is_action_just_pressed(GameAction::Escape, &keyboard_input, &key_map) {
            commands.entity(root).despawn();
        }
    } else if key && *ui_mode == UIMode::Closed {
        map.focus = None;
        spawn_world_map(&mut commands, &assets, &map);
    }
}

/// Spawns the world map over the whole window.
pub fn spawn_world_map(commands: &mut Commands, assets: &AssetServer, map: &WorldMap) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            WorldMapRoot,
            Name::new("World map"),
            UiDialog,
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(WORLD_MAP_TITLE),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            // what is not loaded, all around the world
            BackgroundColor(SLOT),
            Visibility::Visible,
            GlobalZIndex(4),
            DespawnOnExit(GameState::Game),
        ))
        .id();
    let view = commands
        .spawn((
            WorldMapView,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(HINT_BAR_HEIGHT),
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(root),
        ))
        .observe(drag_world_map)
        .observe(zoom_world_map_with_the_wheel)
        .id();
    let picture = commands
        .spawn((
            WorldMapPicture,
            ImageNode::new(map.handle.clone()),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            // the view under it takes the drags and the wheel
            Pickable::IGNORE,
            ChildOf(view),
        ))
        .id();
    let (arrow, size) = map.arrow.clone();
    commands.spawn((
        MinimapArrow,
        WorldMapArrow,
        ImageNode::new(arrow),
        Node {
            position_type: PositionType::Absolute,
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50., -50.)),
        Pickable::IGNORE,
        ChildOf(picture),
    ));

    // the title and the coordinates of the player, at the top-left
    let header = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(SPACE_L),
                top: px(SPACE_L),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                padding: UiRect::axes(px(SPACE_L), px(SPACE_XS)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Text::new(WORLD_MAP_TITLE),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_L).into(),
            ..default()
        },
        TextColor(TEXT),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        MinimapCoords,
        Text::default(),
        debug_font(assets),
        TextColor(TEXT),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    // north, at the top of the view
    commands.spawn((
        Text::new("N"),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_L).into(),
            ..default()
        },
        TextColor(ACCENT),
        Node {
            position_type: PositionType::Absolute,
            top: px(SPACE_L),
            left: percent(50),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50., 0.)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        hint_bar(&WORLD_MAP_HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
}

/// Dragging the map moves it with the cursor, and stops it following the player.
fn drag_world_map(
    drag: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    map: Option<ResMut<WorldMap>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    let (Some(mut map), Ok(player)) = (map, player.single()) else {
        return;
    };
    if drag.button != PointerButton::Primary {
        return;
    }
    let focus = map.focus.unwrap_or(player.translation.xz());
    let moved = drag.delta / ui_scale.0 / map.block_px();
    map.focus = Some(keep_on_the_world(focus - moved, &map));
}

/// The wheel zooms the map around the cursor: the point of the world under it stays there.
fn zoom_world_map_with_the_wheel(
    scroll: On<Pointer<Scroll>>,
    ui_scale: Res<UiScale>,
    map: Option<ResMut<WorldMap>>,
    views: Query<&ComputedNode, With<WorldMapView>>,
    window: Query<&Window, With<PrimaryWindow>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut remainder: Local<f32>,
) {
    let (Some(mut map), Ok(player)) = (map, player.single()) else {
        return;
    };
    let steps = scroll_steps(&mut remainder, scroll.unit, scroll.y);
    let scale = zoomed(map.scale, steps);
    if scale == map.scale {
        return;
    }
    // the view starts at the top-left corner of the window
    let anchor = scroll.pointer_location.position / ui_scale.0
        - view_size(views.single().ok(), window.single().ok(), &ui_scale) / 2.0;
    let focus = map.focus.unwrap_or(player.translation.xz());
    let focus = zoom_focus(focus, anchor, map.block_px(), SCALES[scale]);
    map.scale = scale;
    // around the player, it still follows them
    if map.focus.is_some() || anchor != Vec2::ZERO {
        map.focus = Some(keep_on_the_world(focus, &map));
    }
}

/// `=` and `-` zoom the map around its centre, and Space centres it on the player again.
pub fn world_map_keys(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    map: Option<ResMut<WorldMap>>,
) {
    let Some(mut map) = map else {
        return;
    };
    let mut steps = 0;
    if is_action_just_pressed(GameAction::MinimapZoomIn, &keyboard_input, &key_map) {
        steps += 1;
    }
    if is_action_just_pressed(GameAction::MinimapZoomOut, &keyboard_input, &key_map) {
        steps -= 1;
    }
    let scale = zoomed(map.scale, steps);
    if scale != map.scale {
        map.scale = scale;
    }
    if keyboard_input.just_pressed(KeyCode::Space) {
        map.focus = None;
    }
}

/// The index in `SCALES` `steps` zooms in (or out, when negative) from `scale`.
pub fn zoomed(scale: usize, steps: i32) -> usize {
    (scale as i32 + steps).clamp(0, SCALES.len() as i32 - 1) as usize
}

/// The point at the centre of the view after zooming from `from` to `to` px a block, so that the
/// point `anchor` px from the centre shows the same place of the world.
pub fn zoom_focus(focus: Vec2, anchor: Vec2, from: f32, to: f32) -> Vec2 {
    focus + anchor / from - anchor / to
}

/// The size of the view in logical px, as laid out, or before the layout of the frame the map
/// opens in, the window less the hint bar.
fn view_size(view: Option<&ComputedNode>, window: Option<&Window>, ui_scale: &UiScale) -> Vec2 {
    view.map(|view| view.size() * view.inverse_scale_factor())
        .filter(|size| size.cmpgt(Vec2::ZERO).all())
        .or_else(|| window.map(|window| window.size() / ui_scale.0 - Vec2::Y * HINT_BAR_HEIGHT))
        .unwrap_or_default()
}

/// `focus` kept within the image of the world, so that the map is never dragged away from it.
fn keep_on_the_world(focus: Vec2, map: &WorldMap) -> Vec2 {
    let corner = map.corner.as_vec2();
    focus.clamp(corner, corner + map.size.as_vec2().max(Vec2::ZERO))
}

/// Where the top-left corner of the image goes in a view of `view` logical px centred on the
/// point `focus` of the world, at `block_px` px a block: on whole px, so that every block of a
/// zoom is the same size.
pub fn picture_corner(corner: IVec2, focus: Vec2, view: Vec2, block_px: f32) -> Vec2 {
    (view / 2.0 + (corner.as_vec2() - focus) * block_px).round()
}

/// Draws the world into the image while the map is open, when the surface changed, at most every
/// `REDRAW_EVERY` so that the chunks coming in do not draw it every frame.
pub fn draw_world_map(
    time: Res<Time>,
    map: Option<ResMut<WorldMap>>,
    surface: Option<Res<MinimapSurface>>,
    colors: Option<Res<MinimapColors>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(mut map), Some(surface), Some(colors)) = (map, surface, colors) else {
        return;
    };
    let now = time.elapsed();
    match map.drawn {
        Some((revision, _)) if revision == surface.revision => return,
        Some((_, at)) if now.saturating_sub(at) < REDRAW_EVERY => return,
        _ => {}
    }
    let Some((min, max)) = surface.bounds() else {
        return;
    };
    let corner = min * CHUNK_SIZE;
    let size = ((max - min + IVec2::ONE) * CHUNK_SIZE).as_uvec2();
    let Some(mut image) = images.get_mut(&map.handle) else {
        return;
    };
    if image.size() != size {
        image.resize(Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        });
    }
    let Some(pixels) = image.data.as_mut() else {
        return;
    };
    compose_world(&surface, &colors, corner, size.x, pixels);
    map.corner = corner;
    map.size = size;
    map.drawn = Some((surface.revision, now));
}

/// Places the image in the view, and the arrow of the player and the dots of the other players
/// and of the mobs on it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn place_world_map(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    ui_scale: Res<UiScale>,
    views: Query<&ComputedNode, With<WorldMapView>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut pictures: Query<(Entity, &mut Node), With<WorldMapPicture>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    others: Query<(Entity, &GlobalTransform), (With<Player>, Without<CurrentPlayerMarker>)>,
    mobs: Query<(Entity, &GlobalTransform), With<MobRoot>>,
    mut arrows: Query<&mut Node, (With<WorldMapArrow>, Without<WorldMapPicture>)>,
    mut dots: Query<
        (Entity, &WorldMapDot, &mut Node, &mut Visibility),
        (Without<WorldMapPicture>, Without<WorldMapArrow>),
    >,
) {
    let (Some(map), Ok((picture, mut node)), Ok(player)) =
        (map, pictures.single_mut(), player.single())
    else {
        return;
    };
    let block_px = map.block_px();
    let view = view_size(views.single().ok(), window.single().ok(), &ui_scale);
    let focus = map.focus.unwrap_or(player.translation.xz());
    let corner = picture_corner(map.corner, focus, view, block_px);
    let size = map.size.as_vec2() * block_px;
    set_rect(&mut node, corner, size);

    // in percent of the image, so that the markers move and scale with it
    let at = |position: Vec3| {
        (position.xz() - map.corner.as_vec2()) / map.size.as_vec2().max(Vec2::ONE) * 100.0
    };
    let on_the_world =
        |percent: Vec2| percent.cmpge(Vec2::ZERO).all() && percent.cmple(Vec2::splat(100.0)).all();

    for mut arrow in &mut arrows {
        set_percent(&mut arrow, at(player.translation));
    }

    let targets: Vec<(Entity, Vec3, Color)> = others
        .iter()
        .map(|(entity, transform)| (entity, transform.translation(), TEXT))
        .chain(
            mobs.iter()
                .map(|(entity, transform)| (entity, transform.translation(), DANGER)),
        )
        .collect();
    let mut placed = Vec::new();
    for (dot, WorldMapDot { target }, mut node, mut visibility) in &mut dots {
        let Some(&(_, position, _)) = targets.iter().find(|(entity, ..)| entity == target) else {
            commands.entity(dot).despawn();
            continue;
        };
        placed.push(*target);
        let percent = at(position);
        set_percent(&mut node, percent);
        visibility.set_if_neq(if on_the_world(percent) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for &(target, position, color) in &targets {
        if placed.contains(&target) {
            continue;
        }
        let percent = at(position);
        commands.spawn((
            WorldMapDot { target },
            Node {
                position_type: PositionType::Absolute,
                width: px(DOT_SIZE),
                height: px(DOT_SIZE),
                left: Val::Percent(percent.x),
                top: Val::Percent(percent.y),
                border: UiRect::all(px(1)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50., -50.)),
            BackgroundColor(color),
            BorderColor::all(BEVEL_LO),
            if on_the_world(percent) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
            ChildOf(picture),
        ));
    }
}

/// Moves and sizes `node`, writing only what changed: a write lays the UI out again.
fn set_rect(node: &mut Mut<Node>, corner: Vec2, size: Vec2) {
    let rect = [px(corner.x), px(corner.y), px(size.x), px(size.y)];
    if [node.left, node.top, node.width, node.height] != rect {
        [node.left, node.top, node.width, node.height] = rect;
    }
}

/// Puts `node` at `at` percent of its parent, writing only on a move.
fn set_percent(node: &mut Mut<Node>, at: Vec2) {
    let (left, top) = (Val::Percent(at.x), Val::Percent(at.y));
    if node.left != left || node.top != top {
        node.left = left;
        node.top = top;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_and_stops_at_the_ends() {
        assert_eq!(zoomed(1, 1), 2);
        assert_eq!(zoomed(1, -5), 0);
        assert_eq!(zoomed(2, 9), SCALES.len() - 1);

        // 40 px right of the centre at 2 px a block is 20 blocks east; at 4 px, 10 blocks
        let focus = Vec2::new(100.0, -50.0);
        let anchor = Vec2::new(40.0, 0.0);
        let zoomed = zoom_focus(focus, anchor, 2.0, 4.0);
        assert_eq!(zoomed + anchor / 4.0, focus + anchor / 2.0);
        assert_eq!(zoomed, Vec2::new(110.0, -50.0));
        // around the centre, the centre stays
        assert_eq!(zoom_focus(focus, Vec2::ZERO, 2.0, 8.0), focus);
    }

    #[test]
    fn the_image_is_placed_so_that_the_focus_is_at_the_centre() {
        // an image from (-64, -32); the focus 64 blocks east and 32 south of its corner
        let corner = picture_corner(
            IVec2::new(-64, -32),
            Vec2::ZERO,
            Vec2::new(800.0, 600.0),
            2.0,
        );
        assert_eq!(corner, Vec2::new(400.0 - 128.0, 300.0 - 64.0));
        // on whole px
        let corner = picture_corner(IVec2::ZERO, Vec2::splat(0.3), Vec2::splat(101.0), 1.0);
        assert_eq!(corner, Vec2::splat(50.0));
    }
}
