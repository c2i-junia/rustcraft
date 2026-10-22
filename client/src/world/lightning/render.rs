//! The pass that draws the bolts of lightning (`lightning.wgsl`): the points of their channels
//! in a storage buffer, each piece of channel a ribbon facing the camera, joined to the next by
//! a miter, with a hot core and a soft glow, added to the frame.
//!
//! As the rain's, it runs after the TAA and before the bloom and the tonemapping, into the main
//! texture of the camera of the player: a thin line drawn before the TAA, with no motion
//! vectors, would come up as a faint haze over a few frames, and a bolt lives half a second.
//! With the shaders it is far brighter than white, and the bloom spreads it.

use bevy::anti_alias::taa::temporal_anti_alias;
use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_resource::{
        binding_types::{
            storage_buffer_read_only, texture_depth_2d, texture_depth_2d_multisampled,
            uniform_buffer,
        },
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        BlendComponent, BlendFactor, BlendOperation, BlendState, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, FragmentState, PipelineCache, PrimitiveState,
        RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, ShaderType,
        SpecializedRenderPipeline, SpecializedRenderPipelines, StorageBuffer, TextureFormat,
        UniformBuffer, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
    view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget, ViewUniformOffset, ViewUniforms},
    GpuResourceAppExt, Render, RenderApp, RenderSystems,
};
use bevy::shader::Shader;

use super::Lightning;
use crate::state::GameState;
use crate::world::precipitation::render::PrecipitationView;
use crate::world::shaders::ShaderFeatures;
use crate::world::GraphicsSettings;

const LIGHTNING_SHADER: Handle<Shader> = uuid_handle!("6e1f0b3c-27d4-4a8e-b5c9-d0a4f8e2c7b1");

/// A point of a channel as the pass reads it (`BoltVertex` in `lightning.wgsl`).
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct BoltVertex {
    pub position: Vec3,
    /// The width of the hot core, in blocks.
    pub width: f32,
    /// How bright it is now: its own brightness times that of its strike's strokes.
    pub brightness: f32,
    /// `FIRST` for the first point of a channel, `LAST` for its last.
    pub ends: u32,
}

pub const FIRST: u32 = 1;
pub const LAST: u32 = 2;

/// The uniforms of `lightning.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct LightningParams {
    /// How bright the core and the glow are drawn: far brighter than white with the shaders,
    /// for the bloom, and as bright as white without.
    pub core: f32,
    pub glow: f32,
}

/// The core and the glow with the shaders, and without.
const HDR_BRIGHTNESS: [f32; 2] = [16., 2.6];
const LDR_BRIGHTNESS: [f32; 2] = [1.6, 0.75];

/// The points the pass draws, kept up to date in the main world and handed to the render world.
/// `None` while no bolt is seen: the pass then draws nothing.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct LightningDraw(pub Option<(Vec<BoltVertex>, LightningParams)>);

/// The points of the bolts of `lightning` now, every channel after the other, the branches
/// faded out once their stroke is over.
pub fn bolt_vertices(lightning: &Lightning) -> Vec<BoltVertex> {
    let mut vertices = Vec::new();
    for strike in &lightning.strikes {
        let Some(bolt) = &strike.bolt else {
            continue;
        };
        let channel = strike.strokes.channel(strike.age);
        let branches = strike.strokes.branches(strike.age);
        for branch in &bolt.branches {
            let level = if branch.depth == 0 { channel } else { branches };
            if level <= 0.001 || branch.points.len() < 2 {
                continue;
            }
            let last = branch.points.len() - 1;
            vertices.extend(
                branch
                    .points
                    .iter()
                    .enumerate()
                    .map(|(i, point)| BoltVertex {
                        position: point.position,
                        width: point.width,
                        brightness: point.brightness * level,
                        ends: if i == 0 { FIRST } else { 0 } | if i == last { LAST } else { 0 },
                    }),
            );
        }
    }
    vertices
}

/// The vertices drawn for `points` bolt vertices: six for each piece between two of them,
/// those across two channels folded into a point.
pub fn vertex_count(points: usize) -> u32 {
    6 * points.saturating_sub(1) as u32
}

/// The pass of the bolts, in the render world. It draws for the camera of the player, the
/// `PrecipitationView` that the pass of the rain extracts.
pub fn lightning_render_plugin(app: &mut App) {
    load_internal_asset!(app, LIGHTNING_SHADER, "lightning.wgsl", Shader::from_wgsl);
    app.init_resource::<LightningDraw>()
        .add_plugins(ExtractResourcePlugin::<LightningDraw>::default())
        .add_systems(OnExit(GameState::Game), stop_drawing_lightning)
        .add_systems(
            PostUpdate,
            lightning_draw_system.run_if(in_state(GameState::Game)),
        );
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_gpu_resource::<SpecializedRenderPipelines<LightningPipeline>>()
        .init_gpu_resource::<LightningPipeline>()
        .init_resource::<LightningBuffers>()
        .add_systems(
            Render,
            (
                prepare_lightning_pipelines.in_set(RenderSystems::Prepare),
                prepare_lightning_bind_groups.in_set(RenderSystems::PrepareBindGroups),
            ),
        )
        .add_systems(
            Core3d,
            draw_lightning
                .after(temporal_anti_alias)
                .in_set(Core3dSystems::EarlyPostProcess),
        );
}

/// Tells the pass the bolts seen now.
fn lightning_draw_system(
    lightning: Res<Lightning>,
    graphics: Res<GraphicsSettings>,
    mut draw: ResMut<LightningDraw>,
) {
    let vertices = bolt_vertices(&lightning);
    if vertices.len() < 2 {
        if draw.0.is_some() {
            draw.0 = None;
        }
        return;
    }
    let [core, glow] = if ShaderFeatures::of(graphics.shaders).sky {
        HDR_BRIGHTNESS
    } else {
        LDR_BRIGHTNESS
    };
    draw.0 = Some((vertices, LightningParams { core, glow }));
}

fn stop_drawing_lightning(mut draw: ResMut<LightningDraw>) {
    draw.0 = None;
}

/// The layouts of the bind group of the pass, with the depth of a view drawn without MSAA and
/// with it.
#[derive(Resource)]
pub struct LightningPipeline {
    layout: BindGroupLayoutDescriptor,
    multisampled_layout: BindGroupLayoutDescriptor,
}

impl FromWorld for LightningPipeline {
    fn from_world(_world: &mut World) -> Self {
        let layout = |multisampled: bool| {
            let depth = if multisampled {
                texture_depth_2d_multisampled()
            } else {
                texture_depth_2d()
            };
            BindGroupLayoutDescriptor::new(
                "lightning_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::VERTEX_FRAGMENT,
                    (
                        uniform_buffer::<bevy::render::view::ViewUniform>(true),
                        uniform_buffer::<LightningParams>(false),
                        storage_buffer_read_only::<BoltVertex>(false),
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightningKey {
    format: TextureFormat,
    multisampled: bool,
}

impl SpecializedRenderPipeline for LightningPipeline {
    type Key = LightningKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let (layout, shader_defs) = if key.multisampled {
            (
                self.multisampled_layout.clone(),
                vec!["MULTISAMPLED".into()],
            )
        } else {
            (self.layout.clone(), vec![])
        };
        // Light adds up
        let add = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        RenderPipelineDescriptor {
            label: Some("lightning_pipeline".into()),
            layout: vec![layout],
            vertex: VertexState {
                shader: LIGHTNING_SHADER,
                shader_defs: shader_defs.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![],
            },
            fragment: Some(FragmentState {
                shader: LIGHTNING_SHADER,
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState {
                        color: add,
                        alpha: add,
                    }),
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
pub struct LightningPipelineId(CachedRenderPipelineId);

fn prepare_lightning_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LightningPipeline>>,
    pipeline: Res<LightningPipeline>,
    views: Query<(Entity, &ExtractedView, &Msaa), With<PrecipitationView>>,
) {
    for (entity, view, msaa) in &views {
        let id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            LightningKey {
                format: view.target_format,
                multisampled: msaa.samples() > 1,
            },
        );
        commands.entity(entity).insert(LightningPipelineId(id));
    }
}

/// The uniforms and the points of the pass, on the GPU.
#[derive(Resource, Default)]
pub struct LightningBuffers {
    params: UniformBuffer<LightningParams>,
    points: StorageBuffer<Vec<BoltVertex>>,
}

#[derive(Component)]
pub struct LightningBindGroup {
    bind_group: BindGroup,
    vertices: u32,
}

fn prepare_lightning_bind_groups(
    mut commands: Commands,
    draw: Option<Res<LightningDraw>>,
    pipeline: Res<LightningPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut buffers: ResMut<LightningBuffers>,
    view_uniforms: Res<ViewUniforms>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    views: Query<(Entity, &ViewDepthTexture, &Msaa), With<PrecipitationView>>,
) {
    let drawn = draw.as_ref().and_then(|draw| draw.0.as_ref());
    let Some((points, params)) = drawn else {
        for (entity, ..) in &views {
            commands.entity(entity).remove::<LightningBindGroup>();
        }
        return;
    };
    buffers.params.set(*params);
    buffers.params.write_buffer(&render_device, &render_queue);
    buffers.points.set(points.clone());
    buffers.points.write_buffer(&render_device, &render_queue);
    let (Some(view), Some(params), Some(points_binding)) = (
        view_uniforms.uniforms.binding(),
        buffers.params.binding(),
        buffers.points.binding(),
    ) else {
        return;
    };
    for (entity, depth, msaa) in &views {
        let layout = if msaa.samples() > 1 {
            &pipeline.multisampled_layout
        } else {
            &pipeline.layout
        };
        let bind_group = render_device.create_bind_group(
            "lightning_bind_group",
            &pipeline_cache.get_bind_group_layout(layout),
            &BindGroupEntries::sequential((
                view.clone(),
                params.clone(),
                points_binding.clone(),
                depth.view(),
            )),
        );
        commands.entity(entity).insert(LightningBindGroup {
            bind_group,
            vertices: vertex_count(points.len()),
        });
    }
}

/// Adds the bolts to the frame, after the TAA.
fn draw_lightning(
    view: ViewQuery<(
        &ExtractedCamera,
        &ViewTarget,
        &ViewUniformOffset,
        &LightningPipelineId,
        &LightningBindGroup,
    )>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (camera, target, offset, pipeline, bind_group) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline.0) else {
        return;
    };
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("lightning"),
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
    use crate::world::lightning::{LiveStrike, Strike};

    fn lightning_at(age: f32, x: i32) -> Lightning {
        let mut lightning = Lightning::default();
        let strike = Strike {
            at: IVec3::new(x, 64, 0),
            seed: 9,
        };
        lightning
            .strikes
            .push(LiveStrike::new(strike, Vec3::ZERO, 128.));
        lightning.advance(age);
        lightning
    }

    #[test]
    fn the_pass_is_given_every_channel_of_a_bolt_at_its_brightest() {
        let lightning = lightning_at(0.012, 40);
        let strike = &lightning.strikes[0];
        let bolt = strike.bolt.as_ref().unwrap();
        let vertices = bolt_vertices(&lightning);
        assert_eq!(vertices.len(), bolt.points().count());
        // each channel opens and closes, and the main channel is at its brightest
        let firsts = vertices.iter().filter(|v| v.ends & FIRST != 0).count();
        let lasts = vertices.iter().filter(|v| v.ends & LAST != 0).count();
        assert_eq!((firsts, lasts), (bolt.branches.len(), bolt.branches.len()));
        assert_eq!(vertices[0].ends, FIRST);
        assert_eq!(vertices[0].brightness, 1.);
        assert_eq!(vertices[0].position, bolt.branches[0].points[0].position);
        assert_eq!(
            vertex_count(vertices.len()),
            6 * (vertices.len() as u32 - 1)
        );
    }

    #[test]
    fn a_later_stroke_lights_the_main_channel_alone_and_a_far_strike_draws_nothing() {
        let peaks: Vec<f32> = lightning_at(0., 40).strikes[0].strokes.peaks().collect();
        let lightning = lightning_at(peaks[1], 40);
        let main = lightning.strikes[0].bolt.as_ref().unwrap().branches[0]
            .points
            .len();
        let vertices = bolt_vertices(&lightning);
        assert!(vertices[..main].iter().all(|v| v.brightness > 0.5));
        assert!(vertices[main..].iter().all(|v| v.brightness < 0.04));
        assert!(bolt_vertices(&lightning_at(0.012, 400)).is_empty());
        assert_eq!(vertex_count(0), 0);
        assert_eq!(vertex_count(1), 0);
    }
}
