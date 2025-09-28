use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::world::materials::MaterialResource;
use crate::world::GlobalMaterial;
use crate::KeyMap;
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    key_map: Res<KeyMap>,
) {
    if is_action_just_pressed(
        GameAction::ToggleBlockWireframeDebugMode,
        &keyboard_input,
        &key_map,
    ) && !settings.is_enabled
    {
        settings.is_enabled = true;
        config.global = true;
        let handle = material_resource
            .global_materials
            .get(&GlobalMaterial::Blocks)
            .unwrap();
        let material = materials.get_mut(handle).unwrap();
        material.alpha_mode = AlphaMode::Blend;
        material.base_color.set_alpha(0.3);
        return;
    }

    if is_action_just_pressed(
        GameAction::ToggleBlockWireframeDebugMode,
        &keyboard_input,
        &key_map,
    ) {
        settings.is_enabled = false;
        config.global = false;
        let handle = material_resource
            .global_materials
            .get(&GlobalMaterial::Blocks)
            .unwrap();
        let material = materials.get_mut(handle).unwrap();
        material.alpha_mode = AlphaMode::AlphaToCoverage;
        material.base_color.set_alpha(1.0);
    }
}
