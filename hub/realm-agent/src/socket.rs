//! The Unix socket of the agent: newline-delimited JSON, a response per request, in order.

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};
use rustcraft_agent_protocol::{Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::agent::Agent;

/// The longest request line: a `start` is a few hundred bytes.
const MAX_LINE: u64 = 16 * 1024;

/// Creates the socket at `path` with mode 0660, replacing a socket left by an earlier run.
pub fn bind(path: &Path) -> anyhow::Result<UnixListener> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path)?,
        Ok(_) => bail!("{} exists and is not a socket", path.display()),
        Err(_) => {}
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("could not bind {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
    Ok(listener)
}

/// Answers the connections of `listener` until the process ends.
pub async fn serve(listener: UnixListener, agent: Arc<Agent>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let agent = agent.clone();
                tokio::spawn(async move {
                    if let Err(err) = answer(stream, &agent).await {
                        tracing::warn!("a connection ended: {err}");
                    }
                });
            }
            Err(err) => tracing::warn!("could not accept a connection: {err}"),
        }
    }
}

async fn answer(stream: UnixStream, agent: &Agent) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = (&mut reader)
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut line)
            .await?;
        if read == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n') {
            // Too long, or cut off: the rest of the stream cannot be read in step
            let reason = "a request is one line of at most 16 KiB".into();
            return respond(&mut writer, &Response::Refused { reason }).await;
        }
        let response = match serde_json::from_slice::<Request>(&line) {
            Ok(request) => agent.handle(request).await,
            Err(err) => Response::Refused {
                reason: format!("not a request: {err}"),
            },
        };
        respond(&mut writer, &response).await?;
    }
}

async fn respond(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    response: &Response,
) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(response).expect("a response serializes");
    line.push(b'\n');
    writer.write_all(&line).await
}
