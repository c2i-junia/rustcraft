//! The Bevy Remote Protocol, compiled with the `brp` feature (`just brp`): a JSON-RPC server on
//! http://127.0.0.1:15702, or on the port in `RUSTCRAFT_BRP_PORT`, through which an agent reads
//! and changes the world of the running game, writes input messages to it, and saves
//! screenshots of it. CLAUDE.md lists the calls.
//!
//! Beside the methods Bevy provides (`world.query`, `world.get_components`,
//! `world.write_message`, ...), it adds `rustcraft/screenshot`.

use bevy::prelude::*;
use bevy::remote::{
    http::{RemoteHttpPlugin, DEFAULT_PORT},
    BrpError, BrpResult, RemotePlugin,
};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use serde::Deserialize;
use serde_json::Value;

pub fn brp_plugin(app: &mut App) {
    app.add_plugins((
        RemotePlugin::default().with_method_main(SCREENSHOT_METHOD, screenshot),
        RemoteHttpPlugin::default().with_port(port(std::env::var(PORT_VARIABLE).ok())),
    ));
}

/// The variable that sets the port of the server, for two games to run at once.
const PORT_VARIABLE: &str = "RUSTCRAFT_BRP_PORT";

/// The port in `variable`, or Bevy's default one when it is unset; a wrong value stops the game.
fn port(variable: Option<String>) -> u16 {
    match variable {
        Some(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{PORT_VARIABLE} is not a port: {value:?}")),
        None => DEFAULT_PORT,
    }
}

/// Saves a screenshot of the primary window to the PNG file at `path`, as
/// `{"path": "/tmp/shot.png"}`. It returns at once: the file is written a few frames later.
const SCREENSHOT_METHOD: &str = "rustcraft/screenshot";

#[derive(Deserialize)]
struct ScreenshotParams {
    path: String,
}

fn screenshot(In(params): In<Option<Value>>, mut commands: Commands) -> BrpResult {
    let params: ScreenshotParams =
        serde_json::from_value(params.unwrap_or(Value::Null)).map_err(|error| BrpError {
            code: bevy::remote::error_codes::INVALID_PARAMS,
            message: error.to_string(),
            data: None,
        })?;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(params.path.clone()));
    Ok(Value::String(params.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_is_the_default_one_unless_the_variable_sets_it() {
        assert_eq!(port(None), 15702);
        assert_eq!(port(Some("15703".to_string())), 15703);
    }

    #[test]
    #[should_panic(expected = "RUSTCRAFT_BRP_PORT is not a port")]
    fn a_port_that_is_not_one_stops_the_game() {
        port(Some("http".to_string()));
    }
}
