//! What the server tells the local player of its maps (`shared::world::cartography`): its
//! waypoints, drawn on the minimap and on the world map; the paper maps, drawn into images, the
//! one in hand shown in a corner of the screen; and the frames of maps, which show theirs on
//! their front. A click on the world map names a place there, a right click on a marker takes
//! it away.

use std::collections::{HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::input_focus::InputFocus;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use bevy_renet::RenetClient;
use shared::messages::ClientToServerMessage;
use shared::players::Inventory;
use shared::world::cartography::{
    MapId, PaperMap, PlaceKind, WaypointsView, MARKER_COLORS, MAX_MARKER_NAME, PAPER_MAP_SIDE,
};
use shared::world::{BlockId, ItemId, ItemType};

use super::colors::MinimapColors;
use super::compose::shade;
use super::markers::{dot_corner, map_offset};
use super::surface::MinimapSurface;
use super::world_map::{view_size, WorldMap, WorldMapPicture, WorldMapView};
use super::{column_of, MinimapImage, MinimapView};
use crate::i18n::tr;
use crate::network::SendGameMessageExtension;
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UiDialog;
use crate::ui::theme::{
    ACCENT, BEVEL_LO, BEVEL_WIDTH, DANGER, FONT_M, FONT_S, PANEL, SPACE_S, TEXT,
};
use crate::ui::widgets::button::ButtonVariant;
use crate::ui::widgets::dialog::{
    close_dialog, spawn_form_dialog, CloseRequest, Dialog, DialogSubmit,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::raised_bevel;
use crate::ui::widgets::text_field::{spawn_text_field, value_of, TextField};
use crate::world::ClientWorldMap;

/// The colour of the paper, where its holders never went.
const PAPER: [u8; 3] = [0xD8, 0xC6, 0x9C];
/// The share of the paper in the colour of a column drawn on it.
const PAPER_SHARE: f32 = 0.3;
/// The paper map in hand is drawn this many px a block.
const HELD_SCALE: f32 = 1.5;
/// The side of the mark of a waypoint, in logical px.
const MARK_SIZE: f32 = 8.0;
/// The side of the picture of a framed map, in blocks.
const FRAMED_SIDE: f32 = 14.0 / 16.0;

/// What the server told of the maps.
#[derive(Resource, Default)]
pub struct Cartography {
    pub waypoints: Option<WaypointsView>,
    pub maps: HashMap<MapId, PaperMap>,
    images: HashMap<MapId, Handle<Image>>,
    /// The maps received and not drawn yet.
    undrawn: HashSet<MapId>,
    pub frames: Vec<(IVec3, MapId)>,
    /// Grows with every change of the frames or of the waypoints.
    revision: u64,
}

impl Cartography {
    pub fn set_waypoints(&mut self, view: WaypointsView) {
        self.waypoints = Some(view);
        self.revision += 1;
    }

    pub fn receive_map(&mut self, id: MapId, map: PaperMap) {
        self.maps.insert(id, map);
        self.undrawn.insert(id);
    }

    pub fn set_frames(&mut self, frames: Vec<(IVec3, MapId)>) {
        self.frames = frames;
        self.revision += 1;
    }

    /// The image of the map `id`, blank until it is drawn.
    fn image(&mut self, id: MapId, images: &mut Assets<Image>) -> Handle<Image> {
        self.images
            .entry(id)
            .or_insert_with(|| {
                let side = PAPER_MAP_SIDE as u32;
                let mut image = Image::new_fill(
                    Extent3d {
                        width: side,
                        height: side,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[PAPER[0], PAPER[1], PAPER[2], 255],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                image.sampler = ImageSampler::nearest();
                images.add(image)
            })
            .clone()
    }
}

/// The pixels of `map`, RGBA rows from the north, in the colours of the blocks on paper.
pub fn paper_pixels(map: &PaperMap, colors: &MinimapColors) -> Vec<u8> {
    let water = colors.get(BlockId::Water);
    let corner = map.corner();
    let mut pixels = Vec::with_capacity(map.cells.len() * 4);
    for z in 0..PAPER_MAP_SIDE {
        for x in 0..PAPER_MAP_SIDE {
            let (wx, wz) = (corner.x + x, corner.y + z);
            let color = match map.get(wx, wz) {
                Some(here) => {
                    let color = shade(colors.get(here.block), water, here, map.get(wx, wz - 1));
                    std::array::from_fn(|i| {
                        let mixed = f32::from(color[i]) * (1.0 - PAPER_SHARE)
                            + f32::from(PAPER[i]) * PAPER_SHARE;
                        mixed.round() as u8
                    })
                }
                None => PAPER,
            };
            pixels.extend([color[0], color[1], color[2], 255]);
        }
    }
    pixels
}

/// The kinds of the marks of the waypoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkKind {
    Home,
    Death,
    Found(PlaceKind),
    /// A marker named by the player, by its index.
    Named(usize),
}

/// A waypoint as the maps draw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    pub at: Vec3,
    pub color: Color,
    pub label: String,
}

impl Mark {
    /// Whether the minimap holds it at its edge when it is beyond it.
    fn held(&self) -> bool {
        matches!(self.kind, MarkKind::Home | MarkKind::Death)
    }
}

/// The marks of the waypoints of `view`.
pub fn marks_of(view: &WaypointsView) -> Vec<Mark> {
    let centre = |at: IVec3| at.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let mut marks = vec![Mark {
        kind: MarkKind::Home,
        at: centre(view.home),
        color: ACCENT,
        label: tr(if view.bedroll {
            "waypoint.bedroll"
        } else {
            "waypoint.spawn"
        })
        .to_string(),
    }];
    let waypoints = &view.waypoints;
    if let Some(death) = waypoints.last_death {
        marks.push(Mark {
            kind: MarkKind::Death,
            at: centre(death),
            color: DANGER,
            label: tr("waypoint.death").to_string(),
        });
    }
    for place in &waypoints.found {
        marks.push(Mark {
            kind: MarkKind::Found(place.kind),
            at: centre(place.at),
            color: Color::srgb_u8(0xB0, 0x8C, 0x5A),
            label: tr(place.kind.key()).to_string(),
        });
    }
    for (index, marker) in waypoints.markers.iter().enumerate() {
        let [r, g, b] = MARKER_COLORS[usize::from(marker.color) % MARKER_COLORS.len()];
        marks.push(Mark {
            kind: MarkKind::Named(index),
            at: centre(marker.at),
            color: Color::srgb_u8(r, g, b),
            label: marker.name.clone(),
        });
    }
    marks
}

/// The node of a mark: a square for home, a diamond for the others, of its colour.
fn mark_node(mark: &Mark) -> impl Bundle {
    let turned = !matches!(mark.kind, MarkKind::Home);
    (
        Node {
            position_type: PositionType::Absolute,
            width: px(MARK_SIZE),
            height: px(MARK_SIZE),
            border: UiRect::all(px(1)),
            ..default()
        },
        UiTransform {
            translation: Val2::percent(-50., -50.),
            rotation: if turned {
                Rot2::degrees(45.0)
            } else {
                Rot2::IDENTITY
            },
            ..default()
        },
        BackgroundColor(mark.color),
        BorderColor::all(BEVEL_LO),
    )
}

pub fn cartography_plugin(app: &mut App) {
    app.init_resource::<Cartography>()
        .add_systems(OnEnter(GameState::Game), spawn_held_map)
        .add_systems(OnExit(GameState::Game), |mut commands: Commands| {
            commands.insert_resource(Cartography::default());
        })
        .add_systems(
            Update,
            (
                draw_paper_maps,
                show_held_map,
                place_framed_maps,
                place_minimap_marks,
                place_world_map_marks,
                open_marker_editor,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        );
}

/// Draws the maps received into their images.
fn draw_paper_maps(
    mut cartography: ResMut<Cartography>,
    colors: Option<Res<MinimapColors>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(colors) = colors else {
        return;
    };
    if cartography.undrawn.is_empty() {
        return;
    }
    for id in std::mem::take(&mut cartography.undrawn) {
        let Some(map) = cartography.maps.get(&id) else {
            continue;
        };
        let pixels = paper_pixels(map, &colors);
        let handle = cartography.image(id, &mut images);
        if let Some(mut image) = images.get_mut(&handle) {
            image.data = Some(pixels);
        }
    }
}

/// The paper map in hand, at the bottom right of the screen.
#[derive(Component)]
struct HeldMap;

/// The picture of the map in hand.
#[derive(Component)]
struct HeldMapPicture;

/// The arrow of the player on the map in hand.
#[derive(Component)]
struct HeldMapArrow;

fn spawn_held_map(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let side = PAPER_MAP_SIDE as f32 * HELD_SCALE;
    let (arrow, size) = super::markers::arrow_image(&mut images);
    commands.spawn((
        HeldMap,
        Name::new("Held map"),
        Node {
            position_type: PositionType::Absolute,
            right: px(SPACE_S * 2.0),
            bottom: px(SPACE_S * 2.0),
            padding: UiRect::all(px(SPACE_S)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb_u8(0x8A, 0x6A, 0x3E)),
        raised_bevel(),
        Pickable::IGNORE,
        DespawnOnExit(GameState::Game),
        children![(
            HeldMapPicture,
            ImageNode::default(),
            Node {
                width: px(side),
                height: px(side),
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
            children![(
                HeldMapArrow,
                ImageNode::new(arrow),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(size.x),
                    height: px(size.y),
                    ..default()
                },
                UiTransform::from_translation(Val2::percent(-50., -50.)),
                Pickable::IGNORE,
            )],
        )],
    ));
}

/// The id of the paper map in the selected slot of the hotbar.
fn held_map(inventory: &Inventory, hotbar: Option<&Hotbar>) -> Option<MapId> {
    let stack = inventory.inner.get(&hotbar?.selected)?;
    match (stack.item_id, stack.item_type) {
        (ItemId::FilledMap, ItemType::Map { id }) => Some(id),
        _ => None,
    }
}

/// Shows the paper map in hand, with the player on it, the arrow held at its edge.
#[allow(clippy::type_complexity)]
fn show_held_map(
    inventory: Res<Inventory>,
    hotbar: Query<&Hotbar>,
    mut cartography: ResMut<Cartography>,
    mut images: ResMut<Assets<Image>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut roots: Query<&mut Node, With<HeldMap>>,
    mut pictures: Query<&mut ImageNode, (With<HeldMapPicture>, Without<HeldMapArrow>)>,
    mut arrows: Query<&mut Node, (With<HeldMapArrow>, Without<HeldMap>)>,
) {
    let held = held_map(&inventory, hotbar.single().ok());
    let map = held.and_then(|id| Some((id, cartography.maps.get(&id)?.corner())));
    let display = if map.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut roots {
        if node.display != display {
            node.display = display;
        }
    }
    let Some((id, corner)) = map else {
        return;
    };
    let handle = cartography.image(id, &mut images);
    for mut picture in &mut pictures {
        if picture.image != handle {
            picture.image = handle.clone();
        }
    }
    let Ok(player) = player.single() else {
        return;
    };
    let side = PAPER_MAP_SIDE as f32;
    let at = ((player.translation.xz() - corner.as_vec2()) / side).clamp(Vec2::ZERO, Vec2::ONE);
    for mut arrow in &mut arrows {
        let (left, top) = (Val::Percent(at.x * 100.0), Val::Percent(at.y * 100.0));
        if arrow.left != left || arrow.top != top {
            arrow.left = left;
            arrow.top = top;
        }
    }
}

/// The picture of a framed map.
#[derive(Component)]
struct FramedPicture {
    at: IVec3,
    id: MapId,
}

/// Puts the picture of its map on the front of each frame, and takes away those of the frames
/// gone.
#[allow(clippy::too_many_arguments)]
fn place_framed_maps(
    mut commands: Commands,
    mut cartography: ResMut<Cartography>,
    world: Res<ClientWorldMap>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pictures: Query<(Entity, &FramedPicture)>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    let framed: Vec<(IVec3, MapId, IVec3)> = cartography
        .frames
        .iter()
        .filter_map(|&(at, id)| {
            let block = world.blocks.get_block(at)?;
            (block.id == BlockId::MapFrame).then(|| (at, id, block.direction.offset()))
        })
        .collect();
    let mut shown = Vec::new();
    for (entity, picture) in &pictures {
        if framed
            .iter()
            .any(|&(at, id, _)| at == picture.at && id == picture.id)
        {
            shown.push((picture.at, picture.id));
        } else {
            commands.entity(entity).despawn();
        }
    }
    let mesh = mesh
        .get_or_insert_with(|| meshes.add(Rectangle::new(FRAMED_SIDE, FRAMED_SIDE)))
        .clone();
    for (at, id, front) in framed {
        if shown.contains(&(at, id)) {
            continue;
        }
        let image = cartography.image(id, &mut images);
        let normal = front.as_vec3();
        let centre = at.as_vec3() + Vec3::splat(0.5) + normal * 0.505;
        commands.spawn((
            FramedPicture { at, id },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(image),
                unlit: true,
                ..default()
            })),
            Transform::from_translation(centre)
                .with_rotation(Quat::from_rotation_y(normal.x.atan2(normal.z))),
            DespawnOnExit(GameState::Game),
        ));
    }
}

/// The mark of a waypoint on the minimap.
#[derive(Component)]
struct MinimapMark;

/// Puts the marks of the waypoints on the minimap: home and death held at its edge.
fn place_minimap_marks(
    mut commands: Commands,
    cartography: Res<Cartography>,
    image: Option<Res<MinimapImage>>,
    view: Query<Entity, With<MinimapView>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut marks: Query<(Entity, &mut Node, &mut Visibility), With<MinimapMark>>,
    mut spawned: Local<(u64, Vec<Mark>)>,
) {
    let (Some(image), Ok(view), Ok(player)) = (image, view.single(), player.single()) else {
        return;
    };
    let wanted = cartography
        .waypoints
        .as_ref()
        .map(marks_of)
        .unwrap_or_default();
    if spawned.0 != cartography.revision || spawned.1.len() != marks.iter().len() {
        for (entity, ..) in &marks {
            commands.entity(entity).despawn();
        }
        for mark in &wanted {
            commands.spawn((
                MinimapMark,
                mark_node(mark),
                Visibility::Hidden,
                ChildOf(view),
            ));
        }
        *spawned = (cartography.revision, wanted);
        return;
    }
    let center = column_of(player.translation);
    let zoom = image.blocks_per_pixel();
    for ((_, mut node, mut visibility), mark) in marks.iter_mut().zip(&spawned.1) {
        let offset = map_offset(mark.at, center, zoom);
        match dot_corner(offset, mark.held()) {
            Some(corner) => {
                // the corner of a dot, and a mark is placed by its centre
                let centre = corner + Vec2::splat(super::markers::DOT_SIZE / 2.0);
                if node.left != px(centre.x) || node.top != px(centre.y) {
                    node.left = px(centre.x);
                    node.top = px(centre.y);
                }
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// The mark of a waypoint on the world map, with its name.
#[derive(Component)]
struct WorldMapMark {
    kind: MarkKind,
}

/// Puts the marks of the waypoints on the world map, again when they change or when it opens.
fn place_world_map_marks(
    mut commands: Commands,
    cartography: Res<Cartography>,
    map: Option<Res<WorldMap>>,
    pictures: Query<Entity, With<WorldMapPicture>>,
    marks: Query<Entity, With<WorldMapMark>>,
    assets: Res<AssetServer>,
    mut spawned: Local<Option<(Entity, u64, UVec2, IVec2)>>,
) {
    let (Some(map), Ok(picture)) = (map, pictures.single()) else {
        return;
    };
    let state = (picture, cartography.revision, map.size, map.corner);
    if *spawned == Some(state) {
        return;
    }
    *spawned = Some(state);
    for entity in &marks {
        commands.entity(entity).despawn();
    }
    let Some(view) = &cartography.waypoints else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let size = map.size.as_vec2().max(Vec2::ONE);
    for mark in marks_of(view) {
        let at = (mark.at.xz() - map.corner.as_vec2()) / size * 100.0;
        if at.cmplt(Vec2::ZERO).any() || at.cmpgt(Vec2::splat(100.0)).any() {
            continue;
        }
        commands
            .spawn((
                WorldMapMark { kind: mark.kind },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(at.x),
                    top: Val::Percent(at.y),
                    ..default()
                },
                ChildOf(picture),
            ))
            .with_children(|parent| {
                parent.spawn(mark_node(&mark));
                parent.spawn((
                    Text::new(mark.label.clone()),
                    TextFont {
                        font: font.clone().into(),
                        font_size: px(FONT_S).into(),
                        ..default()
                    },
                    TextColor(TEXT),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: BEVEL_LO,
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(MARK_SIZE),
                        top: px(-MARK_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            })
            .observe(click_mark);
    }
}

/// A right click on a marker takes it away; a click on a mark does not name another place.
fn click_mark(
    mut click: On<Pointer<Click>>,
    marks: Query<&WorldMapMark>,
    mut client: ResMut<RenetClient>,
) {
    click.propagate(false);
    let Ok(mark) = marks.get(click.entity) else {
        return;
    };
    if let (PointerButton::Secondary, MarkKind::Named(index)) = (click.button, mark.kind) {
        client.send_game_message(ClientToServerMessage::RemoveMarker(index as u16));
    }
}

/// The place of the world map the player asked to name, until its editor opens.
#[derive(Resource, Default)]
pub struct MarkerToPlace(pub Option<IVec3>);

/// A click on the world map, not at the end of a drag, names the place under it.
pub fn mark_on_click(
    click: On<Pointer<Click>>,
    ui_scale: Res<UiScale>,
    map: Option<Res<WorldMap>>,
    views: Query<&ComputedNode, With<WorldMapView>>,
    window: Query<&Window, With<PrimaryWindow>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    surface: Option<Res<MinimapSurface>>,
    mut commands: Commands,
) {
    let (Some(map), Ok(player)) = (map, player.single()) else {
        return;
    };
    if click.button != PointerButton::Primary || map.dragged {
        return;
    }
    let anchor = click.pointer_location.position / ui_scale.0
        - view_size(views.single().ok(), window.single().ok(), &ui_scale) / 2.0;
    let focus = map.focus.unwrap_or(player.translation.xz());
    let at = (focus + anchor / map.block_px()).floor().as_ivec2();
    let y = surface
        .and_then(|surface| surface.get(at.x, at.y))
        .map_or(player.translation.y.floor() as i32, |here| here.y + 1);
    commands.insert_resource(MarkerToPlace(Some(IVec3::new(at.x, y, at.y))));
}

/// The open editor of a marker: where it goes, its field, and its colour.
#[derive(Component)]
pub struct MarkerEditor {
    at: IVec3,
    input: Entity,
    color: u8,
}

/// A swatch of the editor of a marker.
#[derive(Component)]
struct Swatch;

const MARKER_HINTS: [Hint; 2] = [
    Hint {
        keys: "Enter",
        action: "Save",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// Opens the editor of the marker asked for: a name and a colour.
fn open_marker_editor(
    to_place: Option<ResMut<MarkerToPlace>>,
    open: Query<(), With<MarkerEditor>>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let Some(mut to_place) = to_place else {
        return;
    };
    let Some(at) = to_place.0.take() else {
        return;
    };
    if !open.is_empty() {
        return;
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    let field = spawn_text_field(
        &mut commands,
        &TextField {
            label: tr("marker.name"),
            placeholder: "",
            max_characters: Some(MAX_MARKER_NAME),
            font: font.clone(),
            font_size: FONT_M,
            reserve_error: false,
        },
    );
    let dialog = Dialog {
        title: tr("marker.title"),
        body: "",
        actions: &[
            (tr("common.cancel"), ButtonVariant::Default),
            (tr("common.save"), ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        &mut commands,
        &dialog,
        &[(tr("marker.name"), field)],
        &MARKER_HINTS,
        font,
        &focus,
    );
    let root = parts.root;
    if let Some(form) = parts.form {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(SPACE_S),
                    ..default()
                },
                AccessibleLabel::new(tr("marker.color")),
                ChildOf(form),
            ))
            .id();
        for (index, [r, g, b]) in MARKER_COLORS.into_iter().enumerate() {
            commands
                .spawn((
                    Swatch,
                    Button,
                    Node {
                        width: px(24),
                        height: px(24),
                        border: UiRect::all(px(BEVEL_WIDTH)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb_u8(r, g, b)),
                    BorderColor::all(if index == 0 { TEXT } else { PANEL }),
                    ChildOf(row),
                ))
                .observe(
                    move |click: On<Pointer<Click>>,
                          mut editors: Query<&mut MarkerEditor>,
                          mut swatches: Query<(Entity, &mut BorderColor), With<Swatch>>| {
                        for mut editor in &mut editors {
                            editor.color = index as u8;
                        }
                        for (entity, mut border) in &mut swatches {
                            *border = BorderColor::all(if entity == click.entity {
                                TEXT
                            } else {
                                PANEL
                            });
                        }
                    },
                );
        }
    }
    commands
        .entity(root)
        .insert((
            MarkerEditor {
                at,
                input: field.input,
                color: 0,
            },
            UiDialog,
            Visibility::Visible,
            DespawnOnExit(GameState::Game),
        ))
        .observe(place_marker_on_submit);
}

/// Sends the marker of the editor to the server, then closes it.
fn place_marker_on_submit(
    submit: On<DialogSubmit>,
    editors: Query<&MarkerEditor, Without<CloseRequest>>,
    texts: Query<&bevy::text::EditableText>,
    mut client: ResMut<RenetClient>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(editor) = editors.get(root) else {
        return;
    };
    let name = value_of(&texts, editor.input);
    if !name.trim().is_empty() {
        client.send_game_message(ClientToServerMessage::PlaceMarker {
            at: editor.at,
            name,
            color: editor.color,
        });
    }
    close_dialog(&mut commands, root, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::cartography::{Surface, Waypoints};

    #[test]
    fn the_paper_shows_where_nobody_went_and_tints_the_rest() {
        let mut map = PaperMap::new(IVec2::ZERO);
        let index = map.index(0, 0).unwrap();
        map.cells[index] = Some(Surface {
            block: BlockId::Stone,
            y: 10,
            water_depth: 0,
        });
        let colors = MinimapColors::default();
        let pixels = paper_pixels(&map, &colors);
        assert_eq!(pixels.len(), (PAPER_MAP_SIDE * PAPER_MAP_SIDE * 4) as usize);
        assert_eq!(&pixels[..4], &[PAPER[0], PAPER[1], PAPER[2], 255]);
        assert_ne!(&pixels[index * 4..index * 4 + 3], &PAPER[..]);
    }

    #[test]
    fn every_waypoint_has_a_mark_and_only_home_and_death_are_held() {
        let mut waypoints = Waypoints {
            last_death: Some(IVec3::new(5, 60, 5)),
            ..default()
        };
        waypoints.find(PlaceKind::Village, IVec3::new(200, 64, 0));
        waypoints.mark("Mine", 3, IVec3::new(-10, 40, 3));
        let view = WaypointsView {
            home: IVec3::ZERO,
            bedroll: true,
            waypoints,
        };
        let marks = marks_of(&view);
        let kinds: Vec<MarkKind> = marks.iter().map(|mark| mark.kind).collect();
        assert_eq!(
            kinds,
            [
                MarkKind::Home,
                MarkKind::Death,
                MarkKind::Found(PlaceKind::Village),
                MarkKind::Named(0)
            ]
        );
        assert_eq!(marks[3].label, "Mine");
        assert!(marks[0].held() && marks[1].held() && !marks[2].held() && !marks[3].held());
    }
}
