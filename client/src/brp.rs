//! The Bevy Remote Protocol, compiled with the `brp` feature (`just brp`): a JSON-RPC server on
//! http://127.0.0.1:15702 through which an agent reads and changes the world of the running
//! game, writes input messages to it, and saves screenshots of it. CLAUDE.md lists the calls.
//!
//! Beside the methods Bevy provides (`world.query`, `world.get_components`,
//! `world.write_message`, ...), it adds `rustcraft/screenshot`.

use bevy::prelude::*;
use bevy::remote::{http::RemoteHttpPlugin, BrpError, BrpResult, RemotePlugin};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use serde::Deserialize;
use serde_json::Value;

pub fn brp_plugin(app: &mut App) {
    app.add_plugins((
        RemotePlugin::default().with_method_main(SCREENSHOT_METHOD, screenshot),
        RemoteHttpPlugin::default(),
    ));
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
