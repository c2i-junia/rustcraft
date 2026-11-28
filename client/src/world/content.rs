//! The content the server sends (`ServerToClientMessage::Content`), at join and each time it
//! read its content files again: a client of a remote server installs it, and every client
//! draws the chunks again when the way they are drawn changed.

use bevy::prelude::*;
use shared::content::{content, install, Content};

use crate::network::session::Dismissal;
use crate::network::{goes_to_local_server, LocalServerAddress, LocalServerHandle, TargetServer};
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
/// alone writes it: its client installs nothing, and takes what the server installed. The
/// client of a remote server installs what it sent, but only once no embedded server runs any
/// more (one left by an earlier session may still be saving), since that server reads the
/// content of the process too: until then the content is kept, to be applied later. When the
/// blocks draw otherwise, the textures are built again and every chunk is meshed again.
pub fn apply_received_content_system(
    mut commands: Commands,
    received: Res<ReceivedContent>,
    (local_server, target, address, mut waiting): (
        Option<Res<LocalServerHandle>>,
        Res<TargetServer>,
        Option<Res<LocalServerAddress>>,
        Local<bool>,
    ),
    (mut applied, mut generation): (ResMut<AppliedContent>, ResMut<ContentGeneration>),
    mut textures: TextureBuild,
    mut world_map: ResMut<ClientWorldMap>,
    mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>,
) {
    if let Some(reason) = received.0.engine_mismatch() {
        commands.remove_resource::<ReceivedContent>();
        // the session ends with the reason (`end_dismissed_session`)
        warn!("The content of the server is refused: {reason}");
        commands.insert_resource(Dismissal(reason));
        return;
    }
    let solo = goes_to_local_server(&target, address.as_deref());
    let embedded_runs = local_server.is_some_and(|handle| {
        handle
            .0
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    });
    if !solo && embedded_runs && target.address.is_some() {
        if !std::mem::replace(&mut *waiting, true) {
            info!("The content of the server is applied once the local server stopped");
        }
        return;
    }
    *waiting = false;
    commands.remove_resource::<ReceivedContent>();
    if !solo && target.address.is_none() {
        // the content of a session that ended
        return;
    }
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

    /// The address of the embedded server of the tests.
    const LOCAL: std::net::SocketAddr =
        std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 9);

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
        world.insert_resource(TargetServer {
            address: Some(LOCAL),
            username: None,
            session_token: None,
            state: crate::network::TargetServerState::FullyReady,
        });
        world.insert_resource(LocalServerAddress(LOCAL));
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

    /// Set in the process `a_remote_server_s_content_is_installed_once_no_local_server_runs`
    /// starts, which installs content other than the game's.
    const OWN_PROCESS: &str = "RUSTCRAFT_REMOTE_CONTENT";

    #[test]
    fn a_remote_server_s_content_is_installed_once_no_local_server_runs() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "world::content::tests::with_a_local_server_still_saving",
                "--ignored",
                "--quiet",
            ])
            .env(OWN_PROCESS, "1")
            .status()
            .unwrap();
        assert!(status.success(), "see the output of the child process");
    }

    #[test]
    #[ignore = "run alone, in its own process, by the test above"]
    fn with_a_local_server_still_saving() {
        assert!(std::env::var_os(OWN_PROCESS).is_some(), "installs content");
        let mut world = client(content().clone());
        // a session on another server, while the server of an earlier one still saves
        world.resource_mut::<TargetServer>().address = Some("192.0.2.1:8000".parse().unwrap());
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        world.insert_resource(LocalServerHandle(Some(std::thread::spawn(move || {
            let _ = stopped.recv();
            Ok(())
        }))));
        let mut harder = content().clone();
        harder.blocks[BlockId::Dirt.index()].break_time = Some(99);
        world.insert_resource(ReceivedContent(harder));

        world
            .run_system_once(apply_received_content_system)
            .unwrap();

        // kept, and the local server still reads the content it started with
        assert!(world.contains_resource::<ReceivedContent>());
        assert_ne!(content().block(BlockId::Dirt).break_time, Some(99));
        assert_eq!(world.resource::<ContentGeneration>().0, u32::MAX);

        stop.send(()).unwrap();
        let handle = world.resource_mut::<LocalServerHandle>().0.take().unwrap();
        handle.join().unwrap().unwrap();
        world
            .run_system_once(apply_received_content_system)
            .unwrap();

        assert!(!world.contains_resource::<ReceivedContent>());
        assert_eq!(content().block(BlockId::Dirt).break_time, Some(99));
        assert_eq!(
            world.resource::<ContentGeneration>().0,
            content().generation
        );
        assert!(!remeshed(&world), "only a property changed");
    }
}
