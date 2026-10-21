//! The rain and the snow: where they fall (`occlusion.rs`, and the climate map, which tells
//! rain from snow), and how they are drawn (`render.rs`, `precipitation.wgsl`).

use bevy::prelude::*;

use crate::camera::eye_under_water;
use crate::state::GameState;
use crate::world::weather::ClientWeather;
use crate::world::{ClientWorldMap, GraphicsSettings, MaterialResource, ShaderQuality};
use occlusion::{forget_occlusion_map, update_occlusion_map_system, OcclusionMap};
use render::{
    precipitation_render_plugin, PrecipitationDraw, PrecipitationParams, PrecipitationSources,
    PrecipitationView,
};

pub mod occlusion;
pub mod render;

/// The rain and the snow, drawn while in game.
pub fn precipitation_plugin(app: &mut App) {
    app.add_plugins(precipitation_render_plugin)
        .init_resource::<OcclusionMap>()
        .add_systems(OnEnter(GameState::Game), forget_occlusion_map)
        .add_systems(OnExit(GameState::Game), stop_drawing_precipitation)
        .add_systems(
            Update,
            (update_occlusion_map_system, precipitation_draw_system)
                .run_if(in_state(GameState::Game)),
        );
}

/// The drops of rain, the flakes of snow and the splashes a quality of the shaders draws at
/// most: without the shaders, still enough rain to be seen, as fewer drops are drawn more opaque.
pub fn counts(quality: ShaderQuality) -> PrecipitationParams {
    let (rain, snow, splashes, opacity) = match quality {
        ShaderQuality::Off => (2_500, 2_500, 160, 1.6),
        ShaderQuality::Low => (3_500, 3_500, 220, 1.35),
        ShaderQuality::Medium => (5_000, 5_000, 300, 1.15),
        ShaderQuality::High => (6_500, 6_500, 400, 1.),
        ShaderQuality::Ultra => (8_000, 8_000, 500, 0.9),
    };
    PrecipitationParams {
        rain,
        snow,
        splashes,
        level: 0.,
        opacity,
    }
}

/// Tells the pass of the rain and the snow what to draw: nothing while nothing falls, nor with
/// the eye under water.
fn precipitation_draw_system(
    weather: Res<ClientWeather>,
    graphics: Res<GraphicsSettings>,
    materials: Res<MaterialResource>,
    occlusion: Res<OcclusionMap>,
    world_map: Res<ClientWorldMap>,
    cameras: Query<&GlobalTransform, With<PrecipitationView>>,
    mut draw: ResMut<PrecipitationDraw>,
) {
    let level = weather.precipitation();
    let seen = cameras
        .iter()
        .next()
        .is_some_and(|eye| !eye_under_water(&world_map.blocks, eye.translation()));
    let sources = (level > 0. && seen).then(|| PrecipitationSources {
        weather: materials.weather.clone(),
        climate: materials.climate_map.clone(),
        occlusion: occlusion.image.clone(),
        params: PrecipitationParams {
            level,
            ..counts(graphics.shaders)
        },
    });
    if draw.0 != sources {
        draw.0 = sources;
    }
}

fn stop_drawing_precipitation(mut draw: ResMut<PrecipitationDraw>) {
    draw.0 = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, BlockId, WeatherKind};

    #[test]
    fn each_quality_draws_more_drops_from_2500_without_the_shaders_to_8000_in_ultra() {
        assert_eq!(counts(ShaderQuality::Off).rain, 2_500);
        assert_eq!(counts(ShaderQuality::Ultra).rain, 8_000);
        for pair in ShaderQuality::ALL.windows(2) {
            let (less, more) = (counts(pair[0]), counts(pair[1]));
            assert!(less.rain < more.rain, "{pair:?}");
            assert!(less.snow < more.snow, "{pair:?}");
            assert!(less.splashes < more.splashes, "{pair:?}");
        }
    }

    #[test]
    fn a_drop_holds_six_vertices_and_a_splash_twelve() {
        let params = PrecipitationParams {
            rain: 10,
            snow: 4,
            splashes: 3,
            level: 1.,
            opacity: 1.,
        };
        assert_eq!(params.vertices(), 6 * 14 + 12 * 3);
    }

    fn app(kind: WeatherKind, eye: Vec3) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(ClientWeather::settled(kind))
            .insert_resource(GraphicsSettings {
                shaders: ShaderQuality::Medium,
                ..default()
            })
            .init_resource::<MaterialResource>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<OcclusionMap>()
            .init_resource::<PrecipitationDraw>()
            .add_systems(Update, precipitation_draw_system);
        let mut map = app.world_mut().resource_mut::<ClientWorldMap>();
        map.blocks
            .insert_chunk(IVec3::new(0, 3, 0), shared::world::Chunk::new());
        map.blocks.set_block(
            IVec3::new(0, 60, 0),
            BlockData::new(BlockId::Water, BlockDirection::Front),
        );
        app.world_mut()
            .spawn((PrecipitationView, GlobalTransform::from_translation(eye)));
        app.update();
        app
    }

    fn drawn(app: &App) -> Option<PrecipitationSources> {
        app.world().resource::<PrecipitationDraw>().0.clone()
    }

    #[test]
    fn the_pass_is_told_the_drops_of_the_quality_and_the_level_of_the_rain() {
        let mut app = app(WeatherKind::Rain, Vec3::new(3.5, 70., 0.5));
        let sources = drawn(&app).expect("rain drawn");
        assert_eq!(
            sources.params,
            PrecipitationParams {
                level: 1.,
                ..counts(ShaderQuality::Medium)
            }
        );
        assert_eq!(
            sources.occlusion,
            app.world().resource::<OcclusionMap>().image
        );
        // as the rain goes, the level follows, and once it is gone nothing is drawn
        let mut weather = ClientWeather::settled(WeatherKind::Rain);
        weather.rain = 0.625;
        app.insert_resource(weather);
        app.update();
        assert!((drawn(&app).unwrap().params.level - 0.5).abs() < 1e-5);
        app.insert_resource(ClientWeather::settled(WeatherKind::Clear));
        app.update();
        assert_eq!(drawn(&app), None);
    }

    #[test]
    fn nothing_falls_for_an_eye_under_water() {
        let app = app(WeatherKind::Thunder, Vec3::new(0.5, 60.3, 0.5));
        assert_eq!(drawn(&app), None);
    }
}
