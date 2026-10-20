use bevy::{
    asset::{load_internal_asset, uuid_handle},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use shared::world::Portal;

const PORTAL_SHADER: Handle<Shader> = uuid_handle!("b7e3a5d1-4c2f-4e8a-9f61-2d7c0b9e5a14");

/// How a `PortalMaterial` draws. Keep in sync with `portal.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortalLook {
    /// What the portal camera sees, read from its image where the fragment is on the screen.
    View = 0,
    /// The colour of the portal: a lone portal, or one seen by a portal camera.
    Color = 1,
    /// The glow on the wall around the opening, from white at its edge to the colour.
    Rim = 2,
}

/// What the portals are drawn with: unlit and unfogged (`portal.wgsl`), as the view they show
/// was lit and fogged by their camera. The box of a portal is drawn by its inner faces.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(PortalKey)]
pub struct PortalMaterial {
    #[uniform(0)]
    pub params: PortalParams,
    /// The image of the portal camera, for `PortalLook::View`.
    #[texture(1)]
    #[sampler(2)]
    pub image: Option<Handle<Image>>,
    pub look: PortalLook,
}

impl PortalMaterial {
    /// A material of `portal`, of colour `color`.
    pub fn new(
        look: PortalLook,
        portal: &Portal,
        color: Color,
        image: Option<Handle<Image>>,
    ) -> Self {
        let normal = portal.normal();
        let center = Vec3::from(portal.frame().translation);
        Self {
            params: PortalParams {
                color: LinearRgba::from(color).to_vec4(),
                plane: normal.extend(-normal.dot(center)),
                look: look as u32,
            },
            image,
            look,
        }
    }
}

/// What makes the pipelines of two portal materials differ.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortalKey {
    rim: bool,
}

impl From<&PortalMaterial> for PortalKey {
    fn from(material: &PortalMaterial) -> Self {
        Self {
            rim: material.look == PortalLook::Rim,
        }
    }
}

/// The uniforms of `portal.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct PortalParams {
    /// The colour of the portal, in linear RGB.
    pub color: Vec4,
    /// The plane of the portal, `(normal, -normal · centre)`: a point is in front of it when
    /// its dot product with `(point, 1)` is positive. A colour is not drawn for a camera behind
    /// it: from there, the front of the portal cannot be seen, and a portal camera always stands
    /// behind its twin, even inside its box.
    pub plane: Vec4,
    pub look: u32,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        PORTAL_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.look {
            PortalLook::Rim => AlphaMode::Blend,
            _ => AlphaMode::Opaque,
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
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The box is seen from inside, through its open front
        descriptor.primitive.cull_mode = if key.bind_group_data.rim {
            None
        } else {
            Some(Face::Front)
        };
        Ok(())
    }
}

/// Makes `PortalMaterial` drawable.
pub fn portal_material_plugin(app: &mut App) {
    load_internal_asset!(app, PORTAL_SHADER, "portal.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<PortalMaterial>::default());
}
