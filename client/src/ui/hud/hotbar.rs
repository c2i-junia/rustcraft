use bevy::prelude::*;

use crate::{
    constants::MAX_HOTBAR_SLOTS, state::GameState, ui::hud::spawn_inventory_cell,
    world::MaterialResource,
};

#[derive(Component)]
pub struct Hotbar {
    pub selected: u32,
}

pub fn setup_hotbar(mut commands: Commands, materials_resource: Res<MaterialResource>) {
    let Some(atlas) = materials_resource.items.as_ref() else {
        error!("The item textures are not loaded: the hotbar cannot be shown");
        return;
    };

    commands
        .spawn((
            Hotbar { selected: 0 },
            DespawnOnExit(GameState::Game),
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
        ))
        .with_children(|bar| {
            for slot in 0..MAX_HOTBAR_SLOTS {
                spawn_inventory_cell(bar, slot, atlas);
            }
        });
}
