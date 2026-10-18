use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::KeyMap;
use crate::world::materials::{ChunkMaterial, MaterialResource};
use crate::world::GlobalMaterial;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::*;

#[derive(Resource)]
pub struct BlockDebugWireframeSettings {
    pub is_enabled: bool,
}

pub fn toggle_wireframe_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<BlockDebugWireframeSettings>,
    mut config: ResMut<WireframeConfig>,
    material_resource: ResMut<MaterialResource>,
    mut materials: ResMut<Assets<ChunkMaterial>>,
    key_map: Res<KeyMap>,
) {
    if !is_action_just_pressed(
        GameAction::ToggleBlockWireframeDebugMode,
        &keyboard_input,
        &key_map,
    ) {
        return;
    }
    settings.is_enabled = !settings.is_enabled;
    config.global = settings.is_enabled;

    // The blocks turn translucent so that the wireframes behind them show through
    let blocks_material = material_resource
        .chunk_materials
        .get(&GlobalMaterial::Blocks)
        .and_then(|handle| materials.get_mut(handle));
    if let Some(mut material) = blocks_material {
        if settings.is_enabled {
            material.base.alpha_mode = AlphaMode::Blend;
            material.base.base_color.set_alpha(0.3);
        } else {
            material.base.alpha_mode = AlphaMode::AlphaToCoverage;
            material.base.base_color.set_alpha(1.0);
        }
    }
}
