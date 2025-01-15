use super::UiDialog;
use crate::constants::{
    HOTBAR_BORDER, HOTBAR_CELL_SIZE, HOTBAR_PADDING, MAX_HOTBAR_SLOTS, MAX_INVENTORY_SLOTS,
};
use crate::ui::hud::{FloatingStack, InventoryCell, InventoryDialog, InventoryRoot};
use crate::world::MaterialResource;
use crate::GameState;
use bevy::{prelude::*, ui::FocusPolicy};

pub fn setup_inventory(mut commands: Commands, materials_resource: Res<MaterialResource>) {
    let atlas = materials_resource.items.as_ref().unwrap();

    // Inventory root: root container for the inventory
    let root = commands
        .spawn((
            UiDialog,
            InventoryRoot,
            StateScoped(GameState::Game),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.),
                    right: Val::Percent(0.),
                    bottom: Val::Percent(0.),
                    top: Val::Percent(0.),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.4)),
                GlobalZIndex(2),
                Visibility::Hidden,
            ),
        ))
        .id();

    let dialog = commands
        .spawn((
            InventoryDialog,
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Percent(7.)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
                BorderRadius::all(Val::Percent(10.)),
            ),
        ))
        .id();

    let inventory_title = commands
        .spawn((
            Text::new("Inventory"),
            TextFont {
                font_size: 24.,
                ..default()
            },
            Node {
                align_content: AlignContent::Center,
                ..default()
            },
        ))
        .id();

    let inventory_grid = commands
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::auto(9),
                margin: UiRect::all(Val::Px(10.)),
                position_type: PositionType::Relative,
                ..default()
            },
            BorderColor(Color::BLACK),
        ))
        .with_children(|builder| {
            for i in MAX_HOTBAR_SLOTS..MAX_INVENTORY_SLOTS {
                builder
                    .spawn((
                        InventoryCell { id: i },
                        (
                            Button,
                            BorderColor(Color::srgb(0.3, 0.3, 0.3)),
                            FocusPolicy::Block,
                            Node {
                                width: Val::Px(HOTBAR_CELL_SIZE),
                                height: Val::Px(HOTBAR_CELL_SIZE),
                                margin: UiRect::ZERO,
                                position_type: PositionType::Relative,
                                padding: UiRect::all(Val::Px(HOTBAR_PADDING)),
                                border: UiRect::all(Val::Px(HOTBAR_BORDER)),
                                ..default()
                            },
                        ),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Test"),
                            TextFont {
                                font_size: 15.,
                                ..default()
                            },
                            Node {
                                position_type: PositionType::Absolute,
                                ..default()
                            },
                        ));
                        btn.spawn((
                            ImageNode::from_atlas_image(
                                atlas.texture.clone_weak(),
                                atlas
                                    .sources
                                    .handle(
                                        atlas.layout.clone_weak(),
                                        if let Some(handle) = atlas.handles.get("test").as_ref() {
                                            handle.id()
                                        } else {
                                            AssetId::default()
                                        },
                                    )
                                    .unwrap_or_default(),
                            ),
                            Node {
                                width: Val::Px(
                                    HOTBAR_CELL_SIZE - 2. * (HOTBAR_PADDING + HOTBAR_BORDER),
                                ),
                                position_type: PositionType::Relative,
                                ..default()
                            },
                            // atlas.clone(), // FIXME: check this
                        ));
                    });
            }
        })
        .id();

    let floating_stack = commands
        .spawn((
            FloatingStack { items: None },
            (
                Node {
                    width: Val::Px(20.),
                    height: Val::Px(20.),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                FocusPolicy::Pass,
            ),
        ))
        .with_children(|btn| {
            btn.spawn(Text::new(""));
            btn.spawn((
                ImageNode::from_atlas_image(
                    atlas.texture.clone_weak(),
                    atlas
                        .sources
                        .handle(
                            atlas.layout.clone_weak(),
                            if let Some(handle) = atlas.handles.get("test").as_ref() {
                                handle.id()
                            } else {
                                AssetId::default()
                            },
                        )
                        .unwrap_or_default(),
                ),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.),
                    right: Val::Percent(0.),
                    bottom: Val::Percent(0.),
                    top: Val::Percent(0.),
                    ..default()
                },
                // atlas.clone(), //FIXME: check this
            ));
        })
        .id();

    commands
        .entity(dialog)
        .add_children(&[inventory_title, inventory_grid]);

    commands
        .entity(root)
        .add_children(&[dialog, floating_stack]);
}
