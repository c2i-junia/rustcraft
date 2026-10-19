//! Another player's game for the tests: a bare client speaking the real protocol, without an app.

use std::net::SocketAddr;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::RenetClient;
use shared::get_customized_server_to_client_channels;
use shared::messages::{AuthRegisterRequest, ClientToServerMessage, ServerToClientMessage};

use crate::network::{open_connection, SendGameMessageExtension};

/// How often the thread of a raw client updates its connection.
const PUMP: Duration = Duration::from_millis(16);

/// How long `exit` waits for the server to close the connection after `Exit`.
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// How far a raw client got.
#[derive(Default, Debug)]
struct Progress {
    connected: bool,
    authenticated: bool,
}

/// What the test asks of the thread.
enum Order {
    Exit,
    Leave,
}

/// Another player's game: a bare netcode client on its own thread, pumped every 16 ms.
pub struct RawClient {
    progress: Arc<(Mutex<Progress>, Condvar)>,
    authenticates: bool,
    orders: Sender<Order>,
    thread: JoinHandle<()>,
}

impl RawClient {
    /// Connects to `server` as `client_id`; authenticates as `name` when given.
    pub fn join(server: SocketAddr, client_id: u64, name: Option<&str>) -> Self {
        let (client, transport) = open_connection(server, client_id).expect("a client socket");
        let progress = Arc::new((Mutex::new(Progress::default()), Condvar::new()));
        let (orders, received) = mpsc::channel();
        let name = name.map(str::to_string);
        let authenticates = name.is_some();
        let shared = progress.clone();
        let thread = std::thread::Builder::new()
            .name(format!("raw client {client_id}"))
            .spawn(move || pump(client, transport, name, &shared, &received))
            .expect("the thread of a raw client");
        Self {
            progress,
            authenticates,
            orders,
            thread,
        }
    }

    /// Waits until connected (and authenticated, when a name was given). Panics after `timeout`.
    #[track_caller]
    pub fn wait_ready(&self, timeout: Duration) {
        let (progress, changed) = &*self.progress;
        let (progress, waited) = changed
            .wait_timeout_while(progress.lock().unwrap(), timeout, |progress| {
                !progress.connected || (self.authenticates && !progress.authenticated)
            })
            .unwrap();
        assert!(
            !waited.timed_out(),
            "the raw client is not ready after {timeout:?}: {progress:?}"
        );
    }

    /// Sends `Exit` — which stops a solo server — then disconnects.
    pub fn exit(self) {
        self.stop(Order::Exit);
    }

    /// Disconnects without a word.
    pub fn leave(self) {
        self.stop(Order::Leave);
    }

    fn stop(self, order: Order) {
        self.orders.send(order).expect("the raw client is running");
        self.thread.join().expect("the raw client stopped");
    }
}

/// Updates the connection every `PUMP` until an order ends it: authenticates as `name` once
/// connected, and drops what the server sends.
fn pump(
    mut client: RenetClient,
    mut transport: NetcodeClientTransport,
    name: Option<String>,
    progress: &(Mutex<Progress>, Condvar),
    orders: &Receiver<Order>,
) {
    let mut asked = false;
    // set once `Exit` is asked: the server closes the connection by this time
    let mut exit_deadline = None;
    let mut exit_sent = false;
    let mut last = Instant::now();
    loop {
        let delta = last.elapsed();
        last = Instant::now();
        client.update(delta);
        // a refused or closed connection is read from `is_disconnected`
        let _ = transport.update(delta, &mut client);

        if client.is_connected() && !asked {
            if let Some(username) = name.clone() {
                client.send_game_message(AuthRegisterRequest { username }.into());
            }
            asked = true;
        }
        let mut authenticated = false;
        for channel in get_customized_server_to_client_channels() {
            while let Some(message) = client.receive_game_message_by_channel(channel.channel_id) {
                authenticated |=
                    matches!(message, Ok(ServerToClientMessage::AuthRegisterResponse(_)));
            }
        }
        {
            let (state, changed) = progress;
            let mut state = state.lock().unwrap();
            state.connected = client.is_connected();
            state.authenticated |= authenticated;
            changed.notify_all();
        }

        match orders.try_recv() {
            // asked to leave, or the test is gone
            Ok(Order::Leave) | Err(TryRecvError::Disconnected) => break,
            Ok(Order::Exit) => exit_deadline = Some(Instant::now() + EXIT_TIMEOUT),
            Err(TryRecvError::Empty) => (),
        }
        if let Some(deadline) = exit_deadline {
            if client.is_disconnected() || Instant::now() >= deadline {
                break;
            }
            // `Exit` can only leave once connected
            if client.is_connected() && !exit_sent {
                client.send_game_message(ClientToServerMessage::Exit);
                exit_sent = true;
            }
        }
        let _ = transport.send_packets(&mut client);
        std::thread::sleep(PUMP);
    }
    transport.disconnect();
}
