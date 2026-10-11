//! The screen shared by the world list and the server list: a title, a bordered scrolling list,
//! a row of text inputs, an "add" button and a "Back to menu" button.
//!
//! The functions here only build the entities and hand their ids back. What the list holds, what
//! the inputs mean and what the buttons do is attached by the screen (`solo.rs`, `multi.rs`),
//! which inserts its own marker components on those entities.

use super::{MenuButtonAction, MenuState, ScrollingList};
use crate::ui::assets::*;
use crate::ui::style::*;
use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInput, TextInputInactive, TextInputPlaceholder, TextInputSettings, TextInputTextColor,
    TextInputTextFont, TextInputValue,
};

pub struct ListScreen<'a> {
    pub menu: MenuState,
    pub title: &'a str,
    pub inputs: &'a [InputField<'a>],
    pub add_label: &'a str,
    /// The input and the buttons take a full line each, instead of sharing two columns.
    pub stacked: bool,
}

#[derive(Default)]
pub struct InputField<'a> {
    pub placeholder: &'a str,
    /// The field is focused when the screen opens.
    pub focused: bool,
    /// The typed text uses the game font and colour; otherwise the input's default text style.
    pub game_font: bool,
}

/// The entities of a spawned screen, for the caller to mark.
pub struct ListScreenParts {
    /// The node the rows are added to (see [`spawn_row`]).
    pub list: Entity,
    /// One per [`ListScreen::inputs`], in the same order.
    pub inputs: Vec<Entity>,
    pub add_button: Entity,
}

/// The text style shared by everything the screen spawns.
#[derive(Clone)]
struct Look {
    font: TextFont,
    color: TextColor,
}

pub fn spawn_list_screen(
    commands: &mut Commands,
    assets: &Res<AssetServer>,
    screen: &ListScreen,
) -> ListScreenParts {
    let look = Look {
        font: TextFont {
            font: load_font(assets).into(),
            font_size: 20.0.into(),
            ..default()
        },
        color: TextColor(TEXT_COLOR),
    };

    let root = spawn_root(commands, assets, screen.menu);
    let title = spawn_title(commands, screen.title, &look);
    let (frame, list) = spawn_list_frame(commands);
    let (grid, inputs, add_button) = spawn_controls(commands, assets, screen, &look);
    commands.entity(root).add_children(&[title, frame, grid]);

    ListScreenParts {
        list,
        inputs,
        add_button,
    }
}

fn spawn_root(commands: &mut Commands, assets: &Res<AssetServer>, menu: MenuState) -> Entity {
    commands
        .spawn((
            DespawnOnExit(menu),
            Node {
                width: Val::Vw(100.0),
                height: Val::Vh(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::horizontal(Val::Percent(20.)),
                row_gap: Val::Percent(2.),
                ..default()
            },
            fill_image_node(load_background_image(assets)),
        ))
        .id()
}

fn spawn_title(commands: &mut Commands, title: &str, look: &Look) -> Entity {
    commands
        .spawn((
            Text::new(title),
            look.font.clone(),
            look.color,
            Node {
                border: UiRect::all(Val::Px(1.)),
                flex_direction: FlexDirection::Column,
                align_content: AlignContent::Center,
                display: Display::Flex,
                ..default()
            },
        ))
        .id()
}

/// The bordered frame that clips the list, which `mouse_scroll` moves inside it.
/// Returns the frame and the list node inside it.
fn spawn_list_frame(commands: &mut Commands) -> (Entity, Entity) {
    let frame = commands
        .spawn((
            BorderColor::all(BACKGROUND_COLOR),
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(50.),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                border: UiRect::all(Val::Px(2.)),
                ..default()
            },
        ))
        .id();
    let list = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.)),
                row_gap: Val::Px(10.),
                ..default()
            },
            ScrollingList { position: 0. },
        ))
        .id();
    commands.entity(frame).add_children(&[list]);
    (frame, list)
}

/// The grid under the list: the text inputs, then the add button and the back button.
/// Returns the grid, the inputs and the add button.
fn spawn_controls(
    commands: &mut Commands,
    assets: &Res<AssetServer>,
    screen: &ListScreen,
    look: &Look,
) -> (Entity, Vec<Entity>, Entity) {
    let mut cell = Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Px(40.0),
        ..default()
    };
    if screen.stacked {
        cell.grid_column = GridPlacement::span(2);
    }

    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.),
            display: Display::Grid,
            grid_template_columns: vec![GridTrack::flex(1.), GridTrack::flex(1.)],
            row_gap: Val::Px(5.),
            column_gap: Val::Px(5.),
            ..default()
        })
        .id();

    let inputs: Vec<Entity> = screen
        .inputs
        .iter()
        .map(|field| spawn_input(commands, field, &cell, look))
        .collect();
    let button_image = load_button_background_large_image(assets);
    let add_button = spawn_button(commands, screen.add_label, &cell, &button_image, look);
    let back_button = spawn_button(commands, "Back to menu", &cell, &button_image, look);
    commands
        .entity(back_button)
        .insert(MenuButtonAction::BackToMainMenu);

    commands.entity(grid).add_children(&inputs);
    commands
        .entity(grid)
        .add_children(&[add_button, back_button]);
    (grid, inputs, add_button)
}

fn spawn_input(commands: &mut Commands, field: &InputField, cell: &Node, look: &Look) -> Entity {
    let mut input = commands.spawn((
        BorderColor::all(BACKGROUND_COLOR),
        BackgroundColor(Color::BLACK),
        cell.clone(),
        TextInput,
        TextInputSettings {
            retain_on_submit: true,
            mask_character: None,
            max_length: None,
        },
        TextInputPlaceholder {
            value: field.placeholder.into(),
            ..default()
        },
        TextInputInactive(!field.focused),
    ));
    if field.game_font {
        input.insert((
            TextInputTextFont(look.font.clone()),
            TextInputTextColor(look.color),
            TextInputValue("".to_string()),
        ));
    } else {
        input.insert(look.font.clone());
    }
    input.id()
}

fn spawn_button(
    commands: &mut Commands,
    label: &str,
    cell: &Node,
    image: &Handle<Image>,
    look: &Look,
) -> Entity {
    commands
        .spawn((
            Button,
            BorderColor::all(Color::BLACK),
            BackgroundColor(BACKGROUND_COLOR),
            cell.clone(),
            fill_image_node(image.clone()),
        ))
        .with_children(|btn| {
            btn.spawn((Text::new(label), look.font.clone(), look.color));
        })
        .id()
}

/// The entities of a row, for the caller to mark: the actions need the row's own id.
pub struct RowParts {
    pub row: Entity,
    pub play: Entity,
    pub delete: Entity,
}

/// Adds a row to `list`: a play icon, a delete icon, the title and, if given, a smaller grey
/// subtitle. The icons are buttons without an action until the caller inserts one.
pub fn spawn_row(
    commands: &mut Commands,
    assets: &Res<AssetServer>,
    list: Entity,
    title: &str,
    subtitle: Option<&str>,
) -> RowParts {
    let font: Handle<Font> = load_font(assets);
    let icon_button_node = Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Percent(80.),
        ..default()
    };
    let icon = Node {
        height: Val::Percent(100.),
        ..default()
    };

    let row = commands
        .spawn((
            BorderColor::all(BACKGROUND_COLOR),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.),
                width: Val::Percent(100.),
                height: Val::Vh(10.),
                padding: UiRect::horizontal(Val::Percent(2.)),
                border: UiRect::all(Val::Px(2.)),
                ..default()
            },
        ))
        .id();

    let mut icon_button = |image: &'static str| {
        commands
            .spawn((Button, icon_button_node.clone()))
            .with_children(|btn| {
                btn.spawn((ImageNode::new(assets.load(image)), icon.clone()));
            })
            .id()
    };
    let play = icon_button("./graphics/play.png");
    let delete = icon_button("./graphics/trash.png");

    let title = commands
        .spawn((
            Text::new(title),
            text_font(font.clone(), 20.0),
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(row).add_children(&[play, delete, title]);
    if let Some(subtitle) = subtitle {
        let subtitle = commands
            .spawn((
                Text::new(subtitle),
                text_font(font, 15.0),
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
            ))
            .id();
        commands.entity(row).add_children(&[subtitle]);
    }
    commands.entity(list).add_children(&[row]);

    RowParts { row, play, delete }
}
