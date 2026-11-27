//! The content the server sends (`ServerToClientMessage::Content`), at join and each time it
//! read its content files again: a client of a remote server installs it, and every client
//! draws the chunks again when the way they are drawn changed.

use bevy::prelude::*;
use shared::content::{content, install, Content};

use crate::network::session::Dismissal;
use crate::network::LocalServerHandle;
use crate::world::{
    rebuild_textures, respawn_every_group, ClientWorldMap, TextureBuild,
    WorldRenderRequestUpdateEvent,
};

/// The content the server sent last, until it is applied.
#[derive(Resource)]
pub struct ReceivedContent(pub Content);

/// The generation of the content this client applied last (`Content::generation`): what the
/// caches built from the content watch (`resource_changed`).
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct ContentGeneration(pub u32);

/// The content the chunks on display and the caches were last made for.
#[derive(Resource)]
pub struct AppliedContent(&'static Content);

impl Default for AppliedContent {
    fn default() -> Self {
        Self(content())
    }
}

/// Applies the content the server sent. The solo server shares the content of the process, and
/// alone writes it: its client installs nothing, and takes what the server installed. When
/// the blocks draw otherwise, the textures are built again and every chunk is meshed again.
pub fn apply_received_content_system(
    mut commands: Commands,
    received: Res<ReceivedContent>,
    local_server: Option<Res<LocalServerHandle>>,
    (mut applied, mut generation): (ResMut<AppliedContent>, ResMut<ContentGeneration>),
    mut textures: TextureBuild,
    mut world_map: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
) {
    commands.remove_resource::<ReceivedContent>();
    if let Some(reason) = received.0.engine_mismatch() {
        // the session ends with the reason (`end_dismissed_session`)
        warn!("The content of the server is refused: {reason}");
        commands.insert_resource(Dismissal(reason));
        return;
    }
    let solo = local_server.is_some_and(|handle| handle.0.is_some());
    let current = if solo || received.0.same_properties(content()) {
        content()
    } else {
        install(received.0.clone())
    };
    if current.same_properties(applied.0) {
        return;
    }
    let redraw = !current.draws_like(applied.0);
    applied.0 = current;
    generation.0 = current.generation;
    info!("Content of generation {} applied", current.generation);
    if redraw {
        rebuild_textures(&mut textures);
        respawn_every_group(&mut world_map, &mut commands, &mut ev_render);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::TextureAtlasLayout;
    use shared::world::{BlockId, ItemId};

    use crate::world::{AtlasHandles, ChunkMaterial, MaterialResource};

    /// A client that applied `applied`, one group of chunks on display, with no textures built.
    fn client(applied: Content) -> World {
        let mut world = World::new();
        for init in [
            World::init_resource::<Assets<Image>>,
            World::init_resource::<Assets<TextureAtlasLayout>>,
            World::init_resource::<Assets<StandardMaterial>>,
            World::init_resource::<Assets<ChunkMaterial>>,
            World::init_resource::<MaterialResource>,
            World::init_resource::<ClientWorldMap>,
            World::init_resource::<Messages<WorldRenderRequestUpdateEvent>>,
        ] {
            init(&mut world);
        }
        world.insert_resource(AtlasHandles::<BlockId>::default());
        world.insert_resource(AtlasHandles::<ItemId>::default());
        world.insert_resource(AppliedContent(Box::leak(Box::new(applied))));
        // none the content of the process has
        world.insert_resource(ContentGeneration(u32::MAX));
        // a solo client: nothing is installed whatever is received
        world.insert_resource(LocalServerHandle(Some(std::thread::spawn(|| Ok(())))));
        let shown = world.spawn_empty().id();
        let mut map = world.resource_mut::<ClientWorldMap>();
        map.chunk_arrived(IVec3::ZERO);
        map.render.values_mut().next().unwrap().entity = Some(shown);
        world
    }

    fn receive(world: &mut World) {
        world.insert_resource(ReceivedContent(content().clone()));
        world
            .run_system_once(apply_received_content_system)
            .unwrap();
    }

    fn remeshed(world: &World) -> bool {
        world
            .resource::<Messages<WorldRenderRequestUpdateEvent>>()
            .iter_current_update_messages()
            .any(|event| *event == WorldRenderRequestUpdateEvent::ChunkToReload(IVec3::ZERO))
    }

    #[test]
    fn content_that_draws_otherwise_meshes_every_chunk_again() {
        let mut brighter = content().clone();
        brighter.blocks[BlockId::Torch.index()].light_emission = 3;
        let mut world = client(brighter);

        receive(&mut world);

        assert!(remeshed(&world));
        assert_eq!(
            world.resource::<ContentGeneration>().0,
            content().generation
        );
        assert!(!world.contains_resource::<ReceivedContent>());
    }

    #[test]
    fn content_that_changes_only_properties_meshes_nothing_again() {
        let mut harder = content().clone();
        harder.blocks[BlockId::Dirt.index()].break_time = Some(99);
        let mut world = client(harder);

        receive(&mut world);

        assert!(!remeshed(&world));
        assert_eq!(
            world.resource::<ContentGeneration>().0,
            content().generation
        );
    }

    #[test]
    fn content_whose_engine_blocks_are_named_otherwise_ends_the_session() {
        let mut world = client(content().clone());
        let mut other = content().clone();
        other.block_names.swap(2, 3);
        world.insert_resource(ReceivedContent(other));
        world
            .run_system_once(apply_received_content_system)
            .unwrap();

        let reason = &world.resource::<Dismissal>().0;
        assert!(
            reason.contains("block 2 is \"stone\" on the server"),
            "{reason}"
        );
        assert!(!remeshed(&world));
        assert_eq!(world.resource::<ContentGeneration>().0, u32::MAX);
    }

    #[test]
    fn the_content_already_applied_changes_nothing() {
        let mut world = client(content().clone());

        receive(&mut world);

        assert!(!remeshed(&world));
        assert_eq!(world.resource::<ContentGeneration>().0, u32::MAX);
    }
}
