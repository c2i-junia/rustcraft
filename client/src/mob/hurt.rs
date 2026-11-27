//! A mob the server says was hurt flashes red for `HURT_FLASH_SECONDS`, and is heard
//! (`MobHurt`, played by `audio`).

use std::collections::HashMap;

use bevy::prelude::*;

use shared::players::melee::Strike;
use shared::world::MobKind;

use super::Mob;

/// How long a hurt mob stays red.
pub const HURT_FLASH_SECONDS: f32 = 0.3;

/// How much of the green and blue of a material a hurt mob keeps.
const FLASH_KEEP: f32 = 0.2;

/// The red the materials of a hurt mob glow with, so that a dark one turns red too.
const FLASH_GLOW: LinearRgba = LinearRgba::rgb(0.35, 0.0, 0.0);

/// A mob of the kind `kind` was hurt, at `at`, by a critical blow or a sweep if `struck` says so.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct MobHurt {
    pub kind: MobKind,
    pub at: Vec3,
    pub struck: Option<Strike>,
}

/// The red copy of each material of the mobs' models, made the first time a mob wearing it is
/// hurt.
#[derive(Resource, Default)]
pub struct HurtMaterials(HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>);

impl HurtMaterials {
    fn red(
        &mut self,
        own: &Handle<StandardMaterial>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.0
            .entry(own.id())
            .or_insert_with(|| {
                let mut red = materials.get(own).cloned().unwrap_or_default();
                let base = red.base_color.to_linear();
                red.base_color = LinearRgba::new(
                    1.0,
                    base.green * FLASH_KEEP,
                    base.blue * FLASH_KEEP,
                    base.alpha,
                )
                .into();
                red.emissive += FLASH_GLOW;
                materials.add(red)
            })
            .clone()
    }
}

/// The own material of a mesh of a hurt mob, while it wears the red copy.
#[derive(Component)]
pub struct OwnMaterial(Handle<StandardMaterial>);

/// Dresses the meshes of each hurt mob in red while `Mob::hurt_seconds` is under
/// `HURT_FLASH_SECONDS`, then in their own materials again. A model loaded while the flash
/// lasts turns red as it appears.
pub fn flash_hurt_mobs(
    mut commands: Commands,
    time: Res<Time>,
    mut mobs: Query<(Entity, &mut Mob)>,
    children: Query<&Children>,
    mut meshes: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&OwnMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut red: ResMut<HurtMaterials>,
) {
    for (entity, mut mob) in &mut mobs {
        let Some(elapsed) = mob.hurt_seconds else {
            continue;
        };
        let elapsed = elapsed + time.delta_secs();
        let flashing = elapsed < HURT_FLASH_SECONDS;
        mob.hurt_seconds = flashing.then_some(elapsed);
        for child in children.iter_descendants(entity) {
            let Ok((mut material, own)) = meshes.get_mut(child) else {
                continue;
            };
            match (flashing, own) {
                (true, None) => {
                    let own = material.0.clone();
                    material.0 = red.red(&own, &mut materials);
                    commands.entity(child).insert(OwnMaterial(own));
                }
                (false, Some(OwnMaterial(own))) => {
                    material.0 = own.clone();
                    commands.entity(child).remove::<OwnMaterial>();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::ServerMob;
    use std::time::Duration;

    /// An app with a hurt pig whose model has one mesh, of a white material, and that mesh.
    fn hurt_pig() -> (App, Entity, Handle<StandardMaterial>) {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<HurtMaterials>()
            .add_systems(Update, flash_hurt_mobs);
        let white = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::from_color(Color::WHITE));
        let mut mob = Mob::of(0, &ServerMob::new(MobKind::Pig, Vec3::ZERO));
        mob.hurt_seconds = Some(0.0);
        let mesh = app.world_mut().spawn(MeshMaterial3d(white.clone())).id();
        let scene = app.world_mut().spawn_empty().add_child(mesh).id();
        app.world_mut().spawn(mob).add_child(scene);
        (app, mesh, white)
    }

    fn worn(app: &App, mesh: Entity) -> Handle<StandardMaterial> {
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(mesh)
            .unwrap()
            .0
            .clone()
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.update();
    }

    #[test]
    fn a_hurt_mob_wears_red_then_its_own_colours_again() {
        let (mut app, mesh, white) = hurt_pig();
        advance(&mut app, 0.0);
        let red = worn(&app, mesh);
        assert_ne!(red, white);
        let color = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&red)
            .unwrap()
            .base_color
            .to_srgba();
        assert!(
            color.red > 0.9 && color.green < 0.5 && color.blue < 0.5,
            "{color:?}"
        );
        advance(&mut app, 0.2);
        assert_eq!(worn(&app, mesh), red);
        advance(&mut app, 0.2);
        assert_eq!(worn(&app, mesh), white);
        // The red copy is made once per material
        let mut mob = app.world_mut().query::<&mut Mob>();
        mob.single_mut(app.world_mut()).unwrap().hurt_seconds = Some(0.0);
        advance(&mut app, 0.0);
        assert_eq!(worn(&app, mesh), red);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 2);
    }
}
