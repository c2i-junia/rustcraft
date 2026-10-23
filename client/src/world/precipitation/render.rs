//! The pass that draws the rain, the snow and the splashes (`precipitation.wgsl`): no mesh and
//! no entity, every drop placed and moved by the vertex shader from its index and the time.
//!
//! It runs in Bevy's `Core3d` schedule after the TAA and before the bloom and the tonemapping,
//! into the main texture of the camera of the player. The TAA keeps most of the frame before
//! for a pixel without motion vectors, which a blended drop cannot write: a thin streak drawn
//! before it would fade into a haze. So the drops are drawn after it, and read the depth of the
//! view themselves to hide behind the world (multisampled without the shaders, which draw with
//! MSAA).

use bevy::anti_alias::taa::temporal_anti_alias;
use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    globals::GlobalsBuffer,
    render_asset::RenderAssets,
    render_resource::{
        binding_types::{
            storage_buffer_read_only, texture_2d, texture_depth_2d, texture_depth_2d_multisampled,
            uniform_buffer,
        },
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, PipelineCache,
        PrimitiveState, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, ShaderType,
        SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat, TextureSampleType,
        UniformBuffer, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
    storage::{GpuShaderBuffer, ShaderBuffer},
    texture::GpuImage,
    view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget, ViewUniformOffset, ViewUniforms},
    GpuResourceAppExt, Render, RenderApp, RenderSystems,
};
use bevy::shader::Shader;

use crate::world::overcast::WeatherShading;

const PRECIPITATION_SHADER: Handle<Shader> = uuid_handle!("a4c7e2d9-58b1-4f03-9e6a-2d71c0b8f5e4");

/// Marks the cameras that the rain and the snow are drawn for: the camera of the player and the
/// cameras of the portals, which show them around the twin, whose depth textures are made
/// readable for them.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct PrecipitationView;

/// A view whose eye is under water, for which no drop falls: the player's camera with its eye
/// in the water, or a portal camera whose twin opens under water (`portal/weather.rs`). Each
/// view by its own eye, so that through a portal between the water and the air the rain falls
/// on the side in the air.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct UnderWaterView;

/// The occlusion map a view's drops stop under, if not the player's: a portal camera's is the
/// one around its twin (`portal/weather.rs`), which the player's, around the player, may not
/// hold.
#[derive(Component, Clone, ExtractComponent)]
pub struct ViewOcclusion(pub Handle<Image>);

/// How many drops of each kind the pass draws at most, and of them how many to show: the
/// uniforms of `precipitation.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct PrecipitationParams {
    /// The drops of rain, the flakes of snow and the splashes the pass holds.
    pub rain: u32,
    pub snow: u32,
    pub splashes: u32,
    /// The part of them shown, the level of the drops (`ClientWeather::precipitation`).
    pub level: f32,
    /// How much more opaque the drops are drawn, for the qualities that draw fewer.
    pub opacity: f32,
    /// The sheets of rain, cylinders around the eye that hold the fine drops far off.
    pub sheets: u32,
}

/// The segments of a sheet of rain around the eye, each a quad (`precipitation.wgsl`).
pub const SHEET_SEGMENTS: u32 = 48;

impl PrecipitationParams {
    /// The vertices the pass draws: six for a drop or a flake, twelve for a splash, its ring
    /// on the ground and its droplets, and six for each segment of a sheet.
    pub fn vertices(&self) -> u32 {
        6 * (self.rain + self.snow) + 12 * self.splashes + 6 * SHEET_SEGMENTS * self.sheets
    }
}

/// What the pass draws with, kept up to date in the main world and handed to the render
/// world: the weather, the climate map, the occlusion map, and the drops. `None` while nothing
/// falls, or nothing is seen: the pass then draws nothing.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct PrecipitationDraw(pub Option<PrecipitationSources>);

#[derive(Clone, Debug, PartialEq)]
pub struct PrecipitationSources {
    pub weather: Handle<ShaderBuffer>,
    pub climate: Handle<Image>,
    pub occlusion: Handle<Image>,
    pub params: PrecipitationParams,
}

/// The pass of the rain and the snow, in the render world.
pub fn precipitation_render_plugin(app: &mut App) {
    load_internal_asset!(
        app,
        PRECIPITATION_SHADER,
        "precipitation.wgsl",
        Shader::from_wgsl
    );
    app.init_resource::<PrecipitationDraw>().add_plugins((
        ExtractResourcePlugin::<PrecipitationDraw>::default(),
        ExtractComponentPlugin::<PrecipitationView>::default(),
        ExtractComponentPlugin::<UnderWaterView>::default(),
        ExtractComponentPlugin::<ViewOcclusion>::default(),
    ));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_gpu_resource::<SpecializedRenderPipelines<PrecipitationPipeline>>()
        .init_gpu_resource::<PrecipitationPipeline>()
        .init_resource::<PrecipitationUniforms>()
        .add_systems(
            Render,
            (
                prepare_precipitation_pipelines.in_set(RenderSystems::Prepare),
                prepare_precipitation_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                forget_views_no_longer_drawn::<PrecipitationBindGroup>
                    .in_set(RenderSystems::PrepareBindGroups),
            ),
        )
        .add_systems(
            Core3d,
            draw_precipitation
                .after(temporal_anti_alias)
                .in_set(Core3dSystems::EarlyPostProcess),
        );
}

/// The layouts of the bind group of the pass, with the depth of a view drawn without MSAA and
/// with it, and the pipelines made from them.
#[derive(Resource)]
pub struct PrecipitationPipeline {
    layout: BindGroupLayoutDescriptor,
    multisampled_layout: BindGroupLayoutDescriptor,
}

impl FromWorld for PrecipitationPipeline {
    fn from_world(_world: &mut World) -> Self {
        let layout = |multisampled: bool| {
            let depth = if multisampled {
                texture_depth_2d_multisampled()
            } else {
                texture_depth_2d()
            };
            BindGroupLayoutDescriptor::new(
                "precipitation_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::VERTEX_FRAGMENT,
                    (
                        uniform_buffer::<bevy::render::view::ViewUniform>(true),
                        uniform_buffer::<bevy::render::globals::GlobalsUniform>(false),
                        storage_buffer_read_only::<WeatherShading>(false),
                        uniform_buffer::<PrecipitationParams>(false),
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        texture_2d(TextureSampleType::Float { filterable: true }),
                        depth,
                    ),
                ),
            )
        };
        Self {
            layout: layout(false),
            multisampled_layout: layout(true),
        }
    }
}

/// What makes two pipelines of the pass differ.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrecipitationKey {
    format: TextureFormat,
    multisampled: bool,
}

impl SpecializedRenderPipeline for PrecipitationPipeline {
    type Key = PrecipitationKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let (layout, shader_defs) = if key.multisampled {
            (
                self.multisampled_layout.clone(),
                vec!["MULTISAMPLED".into()],
            )
        } else {
            (self.layout.clone(), vec![])
        };
        RenderPipelineDescriptor {
            label: Some("precipitation_pipeline".into()),
            layout: vec![layout],
            vertex: VertexState {
                shader: PRECIPITATION_SHADER,
                shader_defs: shader_defs.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![],
            },
            fragment: Some(FragmentState {
                shader: PRECIPITATION_SHADER,
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    // The drops are faint veils of light: their colour comes premultiplied
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::COLOR,
                })],
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..default()
            },
            ..default()
        }
    }
}

#[derive(Component)]
pub struct PrecipitationPipelineId(CachedRenderPipelineId);

fn prepare_precipitation_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PrecipitationPipeline>>,
    pipeline: Res<PrecipitationPipeline>,
    views: Query<(Entity, &ExtractedView, &Msaa), With<PrecipitationView>>,
) {
    for (entity, view, msaa) in &views {
        let id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            PrecipitationKey {
                format: view.target_format,
                multisampled: msaa.samples() > 1,
            },
        );
        commands.entity(entity).insert(PrecipitationPipelineId(id));
    }
}

/// The uniforms of the pass, on the GPU.
#[derive(Resource, Default)]
pub struct PrecipitationUniforms(UniformBuffer<PrecipitationParams>);

#[derive(Component)]
pub struct PrecipitationBindGroup {
    bind_group: BindGroup,
    vertices: u32,
}

fn prepare_precipitation_bind_groups(
    mut commands: Commands,
    draw: Option<Res<PrecipitationDraw>>,
    pipeline: Res<PrecipitationPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut uniforms: ResMut<PrecipitationUniforms>,
    view_uniforms: Res<ViewUniforms>,
    globals: Res<GlobalsBuffer>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    views: Query<
        (
            Entity,
            &ViewDepthTexture,
            &Msaa,
            Has<UnderWaterView>,
            Option<&ViewOcclusion>,
        ),
        With<PrecipitationView>,
    >,
) {
    let sources = draw.as_ref().and_then(|draw| draw.0.as_ref());
    for (entity, depth, msaa, under_water, own_occlusion) in &views {
        let resources = sources.filter(|_| !under_water).and_then(|sources| {
            let occlusion = own_occlusion.map_or(&sources.occlusion, |own| &own.0);
            Some((
                sources,
                buffers.get(&sources.weather)?,
                images.get(occlusion)?,
                images.get(&sources.climate)?,
                view_uniforms.uniforms.binding()?,
                globals.buffer.binding()?,
            ))
        });
        let Some((sources, weather, occlusion, climate, view, globals)) = resources else {
            commands.entity(entity).remove::<PrecipitationBindGroup>();
            continue;
        };
        uniforms.0.set(sources.params);
        uniforms.0.write_buffer(&render_device, &render_queue);
        let Some(params) = uniforms.0.binding() else {
            // Not last frame's drops with last frame's buffers
            commands.entity(entity).remove::<PrecipitationBindGroup>();
            continue;
        };
        let layout = if msaa.samples() > 1 {
            &pipeline.multisampled_layout
        } else {
            &pipeline.layout
        };
        let bind_group = render_device.create_bind_group(
            "precipitation_bind_group",
            &pipeline_cache.get_bind_group_layout(layout),
            &BindGroupEntries::sequential((
                view,
                globals,
                weather.buffer.as_entire_binding(),
                params,
                &occlusion.texture_view,
                &climate.texture_view,
                depth.view(),
            )),
        );
        commands.entity(entity).insert(PrecipitationBindGroup {
            bind_group,
            vertices: sources.params.vertices(),
        });
    }
}

/// Takes the bind group `B` of a pass from the views that are no longer `PrecipitationView`s:
/// Bevy keeps a view's render entity from frame to frame, and the pass, which draws every view
/// with its bind group, would go on drawing with the last one.
pub fn forget_views_no_longer_drawn<B: Component>(
    stale: Query<Entity, (With<B>, Without<PrecipitationView>)>,
    mut commands: Commands,
) {
    for view in &stale {
        commands.entity(view).remove::<B>();
    }
}

/// How many views the pass of the drops drew the last frame, for the benches
/// (`count_views_drawn`).
#[cfg(test)]
pub static VIEWS_DRAWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Counts the views the pass of the drops draws, into `VIEWS_DRAWN`.
#[cfg(test)]
pub fn count_views_drawn(views: Query<(), With<PrecipitationBindGroup>>) {
    VIEWS_DRAWN.store(views.iter().count(), std::sync::atomic::Ordering::Relaxed);
}

/// Draws the rain, the snow and the splashes over the frame, after the TAA.
fn draw_precipitation(
    view: ViewQuery<(
        &ExtractedCamera,
        &ViewTarget,
        &ViewUniformOffset,
        &PrecipitationPipelineId,
        &PrecipitationBindGroup,
    )>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (camera, target, offset, pipeline, bind_group) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline.0) else {
        return;
    };
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("precipitation"),
        color_attachments: &[Some(target.get_unsampled_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group.bind_group, &[offset.offset]);
    if let Some(viewport) = camera.viewport.as_ref() {
        pass.set_camera_viewport(viewport);
    }
    pass.draw(0..bind_group.vertices, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct BindGroup;

    /// A view that stops being a `PrecipitationView` loses the bind group it was drawn with, so
    /// that no pass draws it any more; the others keep theirs.
    #[test]
    fn a_view_no_longer_drawn_loses_its_bind_group() {
        let mut app = App::new();
        app.add_systems(Update, forget_views_no_longer_drawn::<BindGroup>);
        let drawn = app.world_mut().spawn((PrecipitationView, BindGroup)).id();
        let gone = app.world_mut().spawn(BindGroup).id();
        app.update();
        assert!(app.world().entity(drawn).contains::<BindGroup>());
        assert!(!app.world().entity(gone).contains::<BindGroup>());
    }
}
