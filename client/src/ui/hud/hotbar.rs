use bevy::{prelude::*, ui::FocusPolicy};

use crate::{
    constants::{HOTBAR_BORDER, HOTBAR_CELL_SIZE, HOTBAR_PADDING, MAX_HOTBAR_SLOTS},
    ui::hud::InventoryCell,
    world::MaterialResource,
    GameState,
};

#[derive(Component)]
pub struct Hotbar {
    pub selected: u32,
}

pub fn setup_hotbar(mut commands: Commands, materials_resource: Res<MaterialResource>) {
    let atlas = materials_resource.items.as_ref().unwrap();

    commands
        .spawn((
            Hotbar { selected: 0 },
            StateScoped(GameState::Game),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(70.),
                    width: Val::Auto,
                    padding: UiRect::ZERO,
                    border: UiRect::ZERO,
                    margin: UiRect::all(Val::Auto),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 0.3)),
                GlobalZIndex(1),
            ),
        ))
        .with_children(|bar| {
            for i in 0..MAX_HOTBAR_SLOTS {
                bar.spawn((
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
                        TextColor(Color::WHITE),
                        TextFont::from_font_size(15.0),
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        ZIndex(1),
                    ));
                    btn.spawn(((
                        Node {
                            width: Val::Px(
                                HOTBAR_CELL_SIZE - 2. * (HOTBAR_PADDING + HOTBAR_BORDER),
                            ),
                            position_type: PositionType::Relative,
                            ..Default::default()
                        },
                        ImageNode::from_atlas_image(
                            atlas.texture.clone_weak(),
                            atlas
                                .sources
                                .handle(
                                    atlas.layout.clone_weak(),
                                    if let Some(handle) = atlas.handles.get("Dirt").as_ref() {
                                        handle.id()
                                    } else {
                                        AssetId::default()
                                    },
                                )
                                .unwrap_or_default(),
                        ),
                    ),));
                });
            }
        });
}
