use bevy::{
    asset::{load_internal_asset, uuid_handle},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

use super::mesh::ATTRIBUTE_SHADE;

const CLOUDS_SHADER: Handle<Shader> = uuid_handle!("3f6c1d2a-9b7e-4c58-a1d0-5e2b8f4c7a93");

/// What the clouds are drawn with: unlit, seen from both sides, faded out with the horizontal
/// distance to the camera (`clouds.wgsl`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudParams,
}

/// The uniforms of `clouds.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct CloudParams {
    /// The colour of the top faces, in linear RGB; the other faces are darker.
    pub tint: Vec4,
    pub opacity: f32,
    /// The horizontal distance to the camera at which the clouds start to fade out, in blocks.
    pub fade_start: f32,
    /// The horizontal distance to the camera beyond which the clouds are gone, in blocks.
    pub fade_end: f32,
}

impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        CLOUDS_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        CLOUDS_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_SHADE.at_shader_location(1),
        ])?];
        // The faces of a cloud are seen from inside it too
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Makes `CloudMaterial` drawable.
pub fn cloud_material_plugin(app: &mut App) {
    load_internal_asset!(app, CLOUDS_SHADER, "clouds.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<CloudMaterial>::default());
}
