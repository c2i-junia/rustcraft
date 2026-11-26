//! The realm agent, as the hub sees it: something that carries out the requests of
//! `rustcraft-agent-protocol`. [`SocketAgent`] talks to the real one over its Unix socket.

use std::{future::Future, path::PathBuf, pin::Pin, time::Duration};

use rustcraft_agent_protocol::{Request, Response};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

/// The answer of the agent, or why there is none.
pub type AgentFuture<'a> = Pin<Box<dyn Future<Output = Result<Response, String>> + Send + 'a>>;

/// What runs the hosted worlds.
pub trait RealmAgent: Send + Sync {
    /// Sends `request` and waits for its response.
    fn call(&self, request: Request) -> AgentFuture<'_>;
}

/// The longest response line read: `purged` lists volume names.
const MAX_LINE: u64 = 1024 * 1024;

/// The agent at the end of a Unix socket, one connection per request.
pub struct SocketAgent {
    path: PathBuf,
}

impl SocketAgent {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// How long the agent may take: an archive copies a whole world, a stop gives the server
    /// its grace to save.
    fn timeout(request: &Request) -> Duration {
        match request {
            Request::Archive { .. } | Request::Purge => Duration::from_secs(20 * 60),
            _ => Duration::from_secs(3 * 60),
        }
    }

    async fn send(&self, request: &Request) -> Result<Response, String> {
        let stream = UnixStream::connect(&self.path)
            .await
            .map_err(|error| format!("could not reach {}: {error}", self.path.display()))?;
        let (reader, mut writer) = stream.into_split();
        let mut line = serde_json::to_vec(request).expect("a request serializes");
        line.push(b'\n');
        writer
            .write_all(&line)
            .await
            .map_err(|error| format!("could not send the request: {error}"))?;
        let mut answer = Vec::new();
        BufReader::new(reader)
            .take(MAX_LINE)
            .read_until(b'\n', &mut answer)
            .await
            .map_err(|error| format!("could not read the response: {error}"))?;
        serde_json::from_slice(&answer).map_err(|error| format!("not a response: {error}"))
    }
}

impl RealmAgent for SocketAgent {
    fn call(&self, request: Request) -> AgentFuture<'_> {
        Box::pin(async move {
            tokio::time::timeout(Self::timeout(&request), self.send(&request))
                .await
                .unwrap_or_else(|_| Err("the agent did not answer in time".into()))
        })
    }
}

/// Why `response` is not the `done` a request wanted, or nothing when it is.
pub fn failure(response: &Result<Response, String>) -> Option<String> {
    match response {
        Ok(Response::Done) => None,
        Ok(Response::Refused { reason }) => Some(format!("the agent refused: {reason}")),
        Ok(Response::Failed { error }) => Some(format!("the agent failed: {error}")),
        Ok(other) => Some(format!("the agent answered {other:?}")),
        Err(error) => Some(error.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_agent_protocol::RealmState;
    use tokio::net::UnixListener;
    use uuid::Uuid;

    #[tokio::test]
    async fn a_request_is_a_json_line_answered_by_one() {
        let dir = std::env::temp_dir().join(format!("hub-agent-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut line = String::new();
            BufReader::new(reader).read_line(&mut line).await.unwrap();
            writer
                .write_all(b"{\"result\":\"status\",\"state\":{\"state\":\"running\"}}\n")
                .await
                .unwrap();
            line
        });
        let agent = SocketAgent::new(path.clone());
        let realm = Uuid::from_u128(7);
        let response = agent.call(Request::Status { realm }).await;
        assert_eq!(
            response,
            Ok(Response::Status {
                state: RealmState::Running
            })
        );
        assert_eq!(
            server.await.unwrap(),
            format!("{{\"request\":\"status\",\"realm\":\"{realm}\"}}\n")
        );
        std::fs::remove_dir_all(&dir).unwrap();
        let error = agent.call(Request::Purge).await.unwrap_err();
        assert!(error.contains("could not reach"), "{error}");
    }

    #[test]
    fn only_done_is_not_a_failure() {
        assert_eq!(failure(&Ok(Response::Done)), None);
        assert_eq!(
            failure(&Ok(Response::Refused {
                reason: "the world is running".into()
            })),
            Some("the agent refused: the world is running".into())
        );
        assert!(failure(&Err("gone".into())).is_some());
    }
}
