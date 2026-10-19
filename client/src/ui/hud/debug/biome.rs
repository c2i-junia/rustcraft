use crate::player::CurrentPlayerMarker;
use bevy::prelude::*;
use shared::world::TerrainNoises;

/// The terrain noises of the world joined, built from the seed the server sent.
#[derive(Resource)]
pub struct WorldBiomes(pub TerrainNoises);

#[derive(Component)]
pub struct BiomeText;

pub fn biome_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    biomes: Option<Res<WorldBiomes>>,
    query: Query<Entity, With<BiomeText>>,
    mut writer: TextUiWriter,
) {
    let (Ok(transform), Some(biomes)) = (player.single(), biomes) else {
        return;
    };
    let position = transform.translation.floor().as_ivec3();
    let biome = biomes.0.column_shape(position.x, position.z).biome;

    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Biome: {biome:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn the_overlay_shows_the_biome_of_the_column_the_player_stands_in() {
        let mut world = World::new();
        world.init_resource::<bevy::text::TextIterScratch>();
        let text = world.spawn((BiomeText, Text::new("Biome: N/A"))).id();
        world.spawn((
            CurrentPlayerMarker {},
            Transform::from_xyz(-119.6, 70.0, 55.4),
        ));
        world.run_system_once(biome_text_update_system).unwrap();
        // No seed yet: the line is left as it is
        assert_eq!(world.get::<Text>(text).unwrap().0, "Biome: N/A");

        // A forest for the seed 42, pinned by the terrain generation of the server
        world.insert_resource(WorldBiomes(TerrainNoises::new(42)));
        world.run_system_once(biome_text_update_system).unwrap();
        assert_eq!(world.get::<Text>(text).unwrap().0, "Biome: Forest");
    }
}
