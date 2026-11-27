//! The Skin screen, reached from the main menu: the skins shipped with the game and those the
//! player imported, as cards, and beside them the chosen one worn by a model that slowly turns.
//! Selecting a card chooses its skin, saved at once.
//!
//! A PNG dropped on the window while the screen is open is imported: copied to the `skins/`
//! folder of the game folder and chosen, or refused with the reason under the preview.
//!
//! The main menu shows the same model (`spawn_home_preview`).

use crate::i18n::tr;
use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    camera::{visibility::RenderLayers, RenderTarget},
    ecs::system::SystemParam,
    input_focus::tab_navigation::{TabGroup, TabIndex},
    prelude::*,
    render::render_resource::TextureFormat,
    ui::Selected,
    ui_widgets::{observe, Activate},
    window::FileDragAndDrop,
};
use shared::players::skin::{PlayerSkin, BUILT_IN_SKINS};
use shared::players::PLAYER_HEIGHT;
use shared::GameFolderPaths;

use super::background::menu_background;
use super::{follow_focus_with_hints, list_hint_bar, text, ListFocus, ListHints, MenuState};
use crate::audio::{PlaySound, Sound};
use crate::player::model::{skin_material, spawn_player_model};
use crate::player::skin::{
    built_in_skin_path, custom_skins_folder, import_skin, list_custom_skins, skin_image,
    skin_texture, write_skin_choice, ChosenSkin, SkinChoice,
};
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{
    BEVEL_WIDTH, DANGER, FONT_M, FONT_S, FONT_XL, MUTED, PANEL, SLOT, SPACE_L, SPACE_M, SPACE_XL,
    TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::hints::{Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{spawn_list_card, spawn_scroll_list, CardSelected, WheelStep};
use crate::ui::widgets::{raised_bevel, sunken_bevel};

/// The title of the screen, and the label of the button, under the preview of the main menu, that opens it.
pub const SKINS_TITLE: &str = "skins.title";

/// What the screen says of the import, under the preview.
pub const IMPORT_NOTE: &str = "skins.import_note";

/// The layer of the preview's model, its light and its camera: no other camera draws it.
pub const PREVIEW_LAYER: usize = 30;

/// The side of the preview, in pixels.
const PREVIEW_SIZE: u32 = 240;
/// The largest size of the preview of the main menu, in pixels.
const HOME_PREVIEW_SIZE: UVec2 = UVec2::new(300, 400);
/// The width of the preview of the main menu, in percent of the height of the window, and at most.
pub const HOME_PREVIEW_WIDTH_VH: f32 = 21.0;
pub const HOME_PREVIEW_MAX_WIDTH: f32 = HOME_PREVIEW_SIZE.x as f32;
/// How fast the model of the preview turns, in radians a second: a turn in about 12 s.
const PREVIEW_TURN_SPEED: f32 = 0.5;
/// The width of the pane of the preview.
const PREVIEW_PANE_WIDTH: f32 = 280.0;
/// The side of the face shown on a card.
const CARD_FACE: f32 = 32.0;

/// A card of a skin: the skin it chooses, and its texture, which the preview takes.
#[derive(Component)]
pub struct SkinCard {
    pub choice: SkinChoice,
    pub skin: PlayerSkin,
    pub texture: Handle<Image>,
}

/// The list of the skin cards.
#[derive(Component)]
pub struct SkinList;

/// The picture of the preview: what its camera draws.
#[derive(Component)]
pub struct SkinPreview;

/// The model of the preview, which turns.
#[derive(Component)]
pub struct SkinPreviewModel;

/// The material the model of the preview wears.
#[derive(Resource)]
struct PreviewMaterial(Handle<StandardMaterial>);

/// The name of the chosen skin, under the preview.
#[derive(Component)]
pub struct SkinName;

/// The line under the preview that says why a dropped file was refused, hidden while it is
/// empty.
#[derive(Component)]
pub struct SkinMessage;

/// The hint bar of the screen.
#[derive(Component, Default)]
struct SkinsHints;

const BACK_HINT: Hint = Hint {
    keys: "Esc",
    action: "Back",
};

impl ListHints for SkinsHints {
    const CARDS: &'static [Hint] = &[
        Hint {
            keys: "↑↓",
            action: "Choose",
        },
        BACK_HINT,
    ];
    const SCREEN: &'static [Hint] = &[
        Hint {
            keys: "Tab",
            action: "Move",
        },
        Hint {
            keys: "Enter",
            action: "Select",
        },
        BACK_HINT,
    ];
}

/// Asks the screen to list the skins again, the chosen one selected, and to show `error` under
/// the preview.
#[derive(Event)]
struct SkinsChanged {
    error: Option<String>,
}

/// The Skin screen.
pub fn skins_plugin(app: &mut App) {
    app.add_systems(OnEnter(MenuState::Skins), skins_setup)
        .add_systems(
            Update,
            (follow_focus_with_hints::<SkinsHints>, import_dropped_skins)
                .run_if(in_state(MenuState::Skins)),
        )
        .add_systems(
            Update,
            turn_skin_preview.run_if(in_state(MenuState::Skins).or_else(in_state(MenuState::Main))),
        )
        .add_systems(OnExit(MenuState::Skins), |mut commands: Commands| {
            commands.remove_resource::<PreviewMaterial>();
        })
        .add_observer(show_skins);
}

/// Spawns the screen and the scene of its preview, then lists the skins.
fn skins_setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    chosen: Res<ChosenSkin>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::Skins),
            TabGroup::new(0),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(SPACE_XL),
                    right: px(SPACE_XL),
                    top: px(SPACE_XL),
                    bottom: px(HINT_BAR_HEIGHT + SPACE_XL),
                },
                row_gap: px(SPACE_M),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        text(tr(SKINS_TITLE), FONT_XL, TEXT, &font),
        ChildOf(content),
    ));
    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(SPACE_L),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    let scroll = spawn_scroll_list(&mut commands, WheelStep::Item);
    commands.entity(scroll.frame).insert(ChildOf(body));
    commands
        .entity(scroll.list)
        .insert((
            SkinList,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::ListBox)),
            AccessibleLabel::new(tr("skins.list")),
        ))
        .observe(choose_selected_skin);

    // the preview: the model, drawn by a camera of its own into the picture of the pane
    let picture = images.add(Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let material = materials.add(skin_material(skin_texture(
        &chosen.skin,
        &assets,
        &mut images,
    )));
    commands.insert_resource(PreviewMaterial(material.clone()));
    spawn_preview_scene(
        &mut commands,
        &mut meshes,
        material,
        picture.clone(),
        MenuState::Skins,
    );

    let pane = commands
        .spawn((
            Node {
                width: px(PREVIEW_PANE_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(SPACE_L)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                row_gap: px(SPACE_M),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        Node {
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(SLOT),
        sunken_bevel(),
        ChildOf(pane),
        children![(
            SkinPreview,
            ImageNode::new(picture),
            Node {
                width: px(PREVIEW_SIZE as f32),
                height: px(PREVIEW_SIZE as f32),
                ..default()
            },
        )],
    ));
    commands.spawn((
        SkinName,
        text(chosen.choice.label(), FONT_M, TEXT, &font),
        ChildOf(pane),
    ));
    commands.spawn((text(tr(IMPORT_NOTE), FONT_S, MUTED, &font), ChildOf(pane)));
    commands.spawn((
        SkinMessage,
        text("", FONT_S, DANGER, &font),
        Node {
            display: Display::None,
            ..default()
        },
        ChildOf(pane),
    ));

    commands.spawn((
        Node::default(),
        ChildOf(content),
        children![(
            button(tr("common.back"), ButtonVariant::Default, font.clone()),
            observe(
                |_: On<Activate>, mut menu_state: ResMut<NextState<MenuState>>| {
                    menu_state.set(MenuState::Main);
                }
            ),
        )],
    ));
    commands.spawn((list_hint_bar::<SkinsHints>(&assets), ChildOf(root)));
    commands.trigger(SkinsChanged { error: None });
}

/// What the main menu needs to show the model wearing the chosen skin.
#[derive(SystemParam)]
pub struct HomePreview<'w> {
    chosen: Res<'w, ChosenSkin>,
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

/// Spawns under `parent` the model wearing the chosen skin, which slowly turns on the main menu:
/// as wide as `HOME_PREVIEW_WIDTH_VH` percent of the height of the window and a third higher, up
/// to `HOME_PREVIEW_SIZE`.
pub fn spawn_home_preview(
    commands: &mut Commands,
    parent: Entity,
    assets: &AssetServer,
    preview: &mut HomePreview,
) {
    let size = HOME_PREVIEW_SIZE;
    let picture = preview.images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let texture = skin_texture(&preview.chosen.skin, assets, &mut preview.images);
    let material = preview.materials.add(skin_material(texture));
    spawn_preview_scene(
        commands,
        &mut preview.meshes,
        material,
        picture.clone(),
        MenuState::Main,
    );
    commands.spawn((
        SkinPreview,
        ImageNode::new(picture),
        Node {
            width: vh(HOME_PREVIEW_WIDTH_VH),
            height: vh(HOME_PREVIEW_WIDTH_VH * 4.0 / 3.0),
            max_width: px(HOME_PREVIEW_MAX_WIDTH),
            max_height: px(size.y as f32),
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(parent),
    ));
}

/// Spawns the model of the preview in `material`, its light and the camera that draws them into
/// `picture`, all on `PREVIEW_LAYER`, for as long as `screen` shows.
fn spawn_preview_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    picture: Handle<Image>,
    screen: MenuState,
) {
    let layers = RenderLayers::layer(PREVIEW_LAYER);
    let middle = Vec3::Y * PLAYER_HEIGHT / 2.0;
    // the model faces -Z: the camera looks at its face first
    commands.spawn((
        Camera3d::default(),
        Camera {
            // before the menu's camera, which shows the picture the same frame
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(picture.into()),
        Transform::from_xyz(0.0, middle.y + 0.3, -2.7).looking_at(middle, Vec3::Y),
        layers.clone(),
        DespawnOnExit(screen),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 7000.0,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        DespawnOnExit(screen),
    ));
    commands
        .spawn((
            SkinPreviewModel,
            Transform::from_translation(middle),
            Visibility::default(),
            layers.clone(),
            DespawnOnExit(screen),
        ))
        .with_children(|body| {
            spawn_player_model(body, meshes, material, PLAYER_HEIGHT, layers);
        });
}

/// Turns the model of the preview about its height.
fn turn_skin_preview(time: Res<Time>, mut models: Query<&mut Transform, With<SkinPreviewModel>>) {
    for mut transform in &mut models {
        transform.rotate_y(PREVIEW_TURN_SPEED * time.delta_secs());
    }
}

/// Builds the cards again: the skins of the game, then those of the `skins/` folder, the chosen
/// one selected and focused, and shows the chosen skin in the preview and `error` under it.
#[allow(clippy::too_many_arguments)]
fn show_skins(
    changed: On<SkinsChanged>,
    lists: Query<Entity, With<SkinList>>,
    mut names: Query<&mut Text, (With<SkinName>, Without<SkinMessage>)>,
    mut messages: Query<(&mut Text, &mut Node), With<SkinMessage>>,
    mut list_focus: ListFocus,
    chosen: Res<ChosenSkin>,
    paths: Res<GameFolderPaths>,
    preview: Option<Res<PreviewMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(list) = lists.single() else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    commands.entity(list).despawn_related::<Children>();
    let built_in: Vec<_> = BUILT_IN_SKINS
        .iter()
        .map(|name| {
            let texture = assets.load(built_in_skin_path(name));
            let skin = PlayerSkin::BuiltIn(name.to_string());
            (SkinChoice::BuiltIn(name.to_string()), skin, texture)
        })
        .collect();
    let custom: Vec<_> = list_custom_skins(&custom_skins_folder(&paths))
        .into_iter()
        .map(|(file, pixels)| {
            let texture = images.add(skin_image(&pixels));
            (
                SkinChoice::Custom(file),
                PlayerSkin::Custom(pixels),
                texture,
            )
        })
        .collect();
    let mut selected = None;
    for (choice, skin, texture) in built_in.into_iter().chain(custom) {
        let card = spawn_list_card(&mut commands, list, choice.label());
        let kind = match choice {
            SkinChoice::BuiltIn(_) => "",
            SkinChoice::Custom(_) => tr("skins.imported"),
        };
        commands.entity(card).insert(children![
            face(texture.clone()),
            (
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                },
                children![text(choice.label(), FONT_M, TEXT, &font)],
            ),
            text(kind, FONT_S, MUTED, &font),
        ]);
        if choice == chosen.choice {
            selected = Some((card, texture.clone()));
        }
        commands.entity(card).insert(SkinCard {
            choice,
            skin,
            texture,
        });
    }
    // the list sends no `CardSelected` for a card selected here: the preview is set now
    if let Some((card, texture)) = selected {
        commands.entity(card).insert((Selected, TabIndex(0)));
        list_focus.set(card, true);
        if let Some(mut material) = preview.and_then(|preview| materials.get_mut(&preview.0)) {
            material.base_color_texture = Some(texture);
        }
    }
    for mut name in &mut names {
        name.0 = chosen.choice.label().to_string();
    }
    show_message(&mut messages, changed.error.clone());
}

/// The face of a skin, for its card: the front of its head, and over it the front of its hat.
fn face(texture: Handle<Image>) -> impl Bundle {
    let layer = |rect: Rect| {
        (
            ImageNode {
                image: texture.clone(),
                rect: Some(rect),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
        )
    };
    (
        Node {
            width: px(CARD_FACE),
            height: px(CARD_FACE),
            flex_shrink: 0.0,
            ..default()
        },
        children![
            layer(Rect::new(8.0, 8.0, 16.0, 16.0)),
            layer(Rect::new(40.0, 8.0, 48.0, 16.0)),
        ],
    )
}

/// Shows `message` under the preview, or hides the line for `None`.
fn show_message(
    messages: &mut Query<(&mut Text, &mut Node), With<SkinMessage>>,
    message: Option<String>,
) {
    for (mut text, mut node) in messages.iter_mut() {
        node.display = if message.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        text.0 = message.clone().unwrap_or_default();
    }
}

/// Chooses the skin of the card just selected: the preview wears it, and it is saved.
#[allow(clippy::too_many_arguments)]
fn choose_selected_skin(
    selected: On<CardSelected>,
    cards: Query<&SkinCard>,
    mut chosen: ResMut<ChosenSkin>,
    paths: Res<GameFolderPaths>,
    preview: Option<Res<PreviewMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut names: Query<&mut Text, (With<SkinName>, Without<SkinMessage>)>,
    mut messages: Query<(&mut Text, &mut Node), With<SkinMessage>>,
) {
    let Ok(card) = cards.get(selected.card) else {
        return;
    };
    *chosen = ChosenSkin {
        choice: card.choice.clone(),
        skin: card.skin.clone(),
    };
    if let Err(error) = write_skin_choice(&card.choice, &paths) {
        error!("Failed to save the skin: {error}");
    }
    if let Some(mut material) = preview.and_then(|preview| materials.get_mut(&preview.0)) {
        material.base_color_texture = Some(card.texture.clone());
    }
    for mut name in &mut names {
        name.0 = card.choice.label().to_string();
    }
    show_message(&mut messages, None);
}

/// Imports the PNG files dropped on the window: each skin is copied to the `skins/` folder and
/// chosen; a file that is no skin is refused, with the reason under the preview.
fn import_dropped_skins(
    mut drops: MessageReader<FileDragAndDrop>,
    mut chosen: ResMut<ChosenSkin>,
    paths: Res<GameFolderPaths>,
    mut commands: Commands,
    mut sounds: MessageWriter<PlaySound>,
) {
    for drop in drops.read() {
        let FileDragAndDrop::DroppedFile { path_buf, .. } = drop else {
            continue;
        };
        let error = match import_skin(path_buf, &custom_skins_folder(&paths)) {
            Ok((file, pixels)) => {
                let choice = SkinChoice::Custom(file);
                if let Err(error) = write_skin_choice(&choice, &paths) {
                    error!("Failed to save the skin: {error}");
                }
                *chosen = ChosenSkin {
                    choice,
                    skin: PlayerSkin::Custom(pixels),
                };
                None
            }
            Err(error) => {
                warn!("Refused the skin {path_buf:?}: {error:?}");
                Some(error.message())
            }
        };
        // No click goes with a drop: the import is heard
        sounds.write(PlaySound::here(if error.is_none() {
            Sound::SkinImported
        } else {
            Sound::Refused
        }));
        commands.trigger(SkinsChanged { error });
    }
}
