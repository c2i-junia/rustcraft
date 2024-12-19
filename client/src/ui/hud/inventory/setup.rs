use super::UiDialog;
use crate::constants::{
    HOTBAR_BORDER, HOTBAR_CELL_SIZE, HOTBAR_PADDING, MAX_HOTBAR_SLOTS, MAX_INVENTORY_SLOTS,
    TEXTURE_SIZE,
};
use crate::ui::hud::{FloatingStack, InventoryCell, InventoryDialog, InventoryRoot};
use crate::world::MaterialResource;
use crate::GameState;
use bevy::{prelude::*, ui::FocusPolicy};

pub fn setup_inventory(
    mut commands: Commands,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    materials_resource: Res<MaterialResource>,
) {
    let img = materials_resource.items.texture.clone().unwrap();

    let atlas = TextureAtlas {
        layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(TEXTURE_SIZE),
            materials_resource.items.uvs.len() as u32,
            1,
            None,
            None,
        )),
        index: 0,
    };

    // Inventory root : root container for the inventory
    let root = commands
        .spawn((
            UiDialog,
            InventoryRoot,
            StateScoped(GameState::Game),
            NodeBundle {
                background_color: BackgroundColor(Color::BLACK.with_alpha(0.4)),
                // Z-index of 2 : displayed above game & HUD, but under everything else
                z_index: ZIndex::Global(2),
                visibility: Visibility::Hidden,
                style: Node {
                    position_type: PositionType::Absolute,
                    // Cover whole screen as a dark backdrop
                    left: Val::Percent(0.),
                    right: Val::Percent(0.),
                    bottom: Val::Percent(0.),
                    top: Val::Percent(0.),
                    // Align children at its center
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();

    let dialog = commands
        .spawn((
            InventoryDialog,
            NodeBundle {
                background_color: BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
                border_radius: BorderRadius::all(Val::Percent(10.)),
                style: Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Percent(7.)),
                    ..default()
                },
                ..default()
            },
        ))
        .id();

    let inventory_title = commands
        .spawn(TextBundle {
            text: Text::from_section(
                "Inventory",
                TextStyle {
                    font_size: 24.,
                    ..Default::default()
                },
            ),
            style: Node {
                align_content: AlignContent::Center,
                ..Default::default()
            },
            ..Default::default()
        })
        .id();

    let inventory_grid = commands
        .spawn(NodeBundle {
            style: Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::auto(9),
                margin: UiRect::all(Val::Px(10.)),
                position_type: PositionType::Relative,
                ..Default::default()
            },
            border_color: BorderColor(Color::BLACK),
            ..Default::default()
        })
        .with_children(|builder| {
            for i in MAX_HOTBAR_SLOTS..MAX_INVENTORY_SLOTS {
                builder
                    .spawn((
                        InventoryCell { id: i },
                        ButtonBundle {
                            border_color: BorderColor(Color::srgb(0.3, 0.3, 0.3)),
                            focus_policy: FocusPolicy::Block,
                            style: Node {
                                width: Val::Px(HOTBAR_CELL_SIZE),
                                height: Val::Px(HOTBAR_CELL_SIZE),
                                margin: UiRect::ZERO,
                                position_type: PositionType::Relative,
                                padding: UiRect::all(Val::Px(HOTBAR_PADDING)),
                                border: UiRect::all(Val::Px(HOTBAR_BORDER)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ))
                    .with_children(|btn| {
                        btn.spawn(TextBundle {
                            text: Text::from_section(
                                "Test",
                                TextStyle {
                                    font_size: 15.,
                                    ..Default::default()
                                },
                            ),
                            style: Node {
                                position_type: PositionType::Absolute,
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                        btn.spawn((
                            ImageBundle {
                                z_index: ZIndex::Local(-1),
                                style: Node {
                                    width: Val::Px(
                                        HOTBAR_CELL_SIZE - 2. * (HOTBAR_PADDING + HOTBAR_BORDER),
                                    ),
                                    position_type: PositionType::Relative,
                                    ..Default::default()
                                },
                                image: UiImage {
                                    texture: img.clone_weak(),
                                    ..default()
                                },
                                ..Default::default()
                            },
                            atlas.clone(),
                        ));
                    });
            }
        })
        .id();

    let floating_stack = commands
        .spawn((
            FloatingStack { items: None },
            NodeBundle {
                focus_policy: FocusPolicy::Pass,
                style: Node {
                    width: Val::Px(20.),
                    height: Val::Px(20.),
                    position_type: PositionType::Absolute,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                "",
                TextStyle {
                    font_size: 15.,
                    ..Default::default()
                },
            ));
            btn.spawn((
                ImageBundle {
                    z_index: ZIndex::Local(-1),
                    style: Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(0.),
                        right: Val::Percent(0.),
                        bottom: Val::Percent(0.),
                        top: Val::Percent(0.),
                        ..Default::default()
                    },
                    image: UiImage {
                        texture: img.clone_weak(),
                        ..default()
                    },
                    ..Default::default()
                },
                atlas.clone(),
            ));
        })
        .id();

    commands
        .entity(dialog)
        .push_children(&[inventory_title, inventory_grid]);

    commands
        .entity(root)
        .push_children(&[dialog, floating_stack]);
}
