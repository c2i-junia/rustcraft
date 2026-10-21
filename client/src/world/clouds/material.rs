use bevy::{
    asset::{load_internal_asset, uuid_handle},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, ColorWrites, RenderPipelineDescriptor, ShaderType,
        SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

use super::mesh::{ATTRIBUTE_DENSITIES, ATTRIBUTE_SHADE};

const CLOUDS_SHADER: Handle<Shader> = uuid_handle!("3f6c1d2a-9b7e-4c58-a1d0-5e2b8f4c7a93");

/// What the clouds are drawn with: unlit, seen from both sides, faded out with the horizontal
/// distance to the camera (`clouds.wgsl`).
///
/// Blended faces behind one another would add up, a cloud seen through its own faces looking
/// thicker than one seen through a single face. So the clouds are drawn twice, as Minecraft
/// does: a first time into the depth only, then in colour where their depth is the one drawn,
/// so that only the nearest face is coloured.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
#[bind_group_data(CloudKey)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudParams,
    /// Whether this is the first drawing, into the depth only.
    pub depth_only: bool,
}

/// What makes the pipelines of two cloud materials differ.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudKey {
    depth_only: bool,
}

impl From<&CloudMaterial> for CloudKey {
    fn from(material: &CloudMaterial) -> Self {
        Self {
            depth_only: material.depth_only,
        }
    }
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
    /// The density above which a cell is full, which grows the clouds in rain
    /// (`field::threshold`).
    pub threshold: f32,
    /// The colour of the clouds under a full overcast (`overcast_cloud_color`), as on screen,
    /// and in `w` the overcast level, by which it replaces the tint.
    pub overcast: Vec4,
    /// The overcast sky at the horizon (`Overcast::colors`): under the overcast, the clouds far
    /// off fade into it.
    pub haze: Vec4,
}

impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        CLOUDS_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        CLOUDS_SHADER.into()
    }

    /// Both drawings are among the transparent meshes, after the terrain, which they let
    /// through.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// Sorted just before the colour, which has the same mesh at the same place.
    fn depth_bias(&self) -> f32 {
        if self.depth_only {
            -1.
        } else {
            0.
        }
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
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_SHADE.at_shader_location(1),
            ATTRIBUTE_DENSITIES.at_shader_location(2),
        ])?];
        // The faces of a cloud are seen from inside it too
        descriptor.primitive.cull_mode = None;
        if key.bind_group_data.depth_only {
            let targets = descriptor.fragment.iter_mut().flat_map(|f| &mut f.targets);
            for target in targets.flatten() {
                target.blend = None;
                target.write_mask = ColorWrites::empty();
            }
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_write_enabled = Some(true);
            }
        }
        // The colour, as the transparent meshes, passes where its depth is equal to the one
        // drawn first: the reversed depth compares with `GreaterEqual`
        Ok(())
    }
}

/// Makes `CloudMaterial` drawable.
pub fn cloud_material_plugin(app: &mut App) {
    load_internal_asset!(app, CLOUDS_SHADER, "clouds.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<CloudMaterial>::default());
}
