//! Route-matching trie and the synthetic SSR fixture payload server.

use deno_core::anyhow::{Context, Result};
use std::collections::BTreeMap;

/// A registered route pattern; `:name` segments capture parameters.
#[derive(Debug, Default)]
pub struct RouteTrie {
    root: Node,
}

#[derive(Debug, Default)]
struct Node {
    children: BTreeMap<String, Node>,
    param: Option<Box<Node>>,
    handler: Option<String>,
}

impl RouteTrie {
    /// Inserts `handler` under a pattern like `/users/:id/posts`.
    pub fn insert(&mut self, pattern: &str, handler: &str) {
        let mut node = &mut self.root;
        for segment in split_segments(pattern) {
            if let Some(name) = segment.strip_prefix(':') {
                let param = node.param.get_or_insert_with(|| Box::new(Node::default()));
                param.handler = None;
                // Remember the parameter name on the parent edge implicitly;
                // names are positional in lookups.
                let _ = name;
                node = param.as_mut();
            } else {
                node = node.children.entry(segment.to_owned()).or_default();
            }
        }
        node.handler = Some(handler.to_owned());
    }

    /// Matches an absolute path, returning the handler and captured params.
    pub fn lookup(&self, path: &str) -> Option<(&str, Vec<(String, String)>)> {
        let mut params = Vec::new();
        let mut node = &self.root;
        for segment in split_segments(path) {
            let next = node.children.get(segment);
            if let Some(static_node) = next {
                node = static_node;
                continue;
            }
            match &node.param {
                Some(param_node) => {
                    params.push((format!("param{}", params.len() + 1), segment.to_owned()));
                    node = param_node;
                }
                None => return None,
            }
        }
        node.handler.as_deref().map(|handler| (handler, params))
    }
}

fn split_segments(pattern: &str) -> impl Iterator<Item = &str> {
    pattern.split('/').filter(|segment| !segment.is_empty())
}

/// Serves one JSON payload on an ephemeral localhost port until stopped.
pub struct FixtureServer {
    pub port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl FixtureServer {
    /// Starts a single-route HTTP/1.1 server answering GET with `payload`.
    pub async fn start(payload: String) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind SSR fixture server")?;
        let port = listener.local_addr().context("fixture address")?.port();
        let (shutdown, mut shutdown_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { continue };
                        serve_once(socket, &payload);
                    }
                }
            }
        });
        Ok(Self { port, shutdown })
    }

    pub fn stop(&self) {
        let _ = self.shutdown.send(true);
    }
}

fn serve_once(mut socket: tokio::net::TcpStream, payload: &str) {
    let body = format!("{{\"message\":\"{payload}\"}}");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut scratch = [0_u8; 1024];
        let _ = socket.read(&mut scratch).await;
        let _ = socket.write_all(response.as_bytes()).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_routes_match_exactly() {
        let mut trie = RouteTrie::default();
        trie.insert("/about", "about-handler");
        assert_eq!(trie.lookup("/about").map(|(h, _)| h), Some("about-handler"));
        assert!(trie.lookup("/about/").is_some(), "trailing slash tolerated");
        assert_eq!(trie.lookup("/nope"), None);
    }

    #[test]
    fn param_segments_capture_values() {
        let mut trie = RouteTrie::default();
        trie.insert("/users/:id", "user-handler");
        let (handler, params) = trie.lookup("/users/42").expect("match");
        assert_eq!(handler, "user-handler");
        assert_eq!(params, vec![("param1".to_owned(), "42".to_owned())]);
    }

    #[test]
    fn static_wins_over_param_at_same_depth() {
        let mut trie = RouteTrie::default();
        trie.insert("/users/new", "new-handler");
        trie.insert("/users/:id", "user-handler");
        assert_eq!(
            trie.lookup("/users/new").map(|(h, _)| h),
            Some("new-handler")
        );
        assert_eq!(
            trie.lookup("/users/17").map(|(h, _)| h),
            Some("user-handler")
        );
    }

    #[test]
    fn deep_patterns_match_full_length_only() {
        let mut trie = RouteTrie::default();
        trie.insert("/api/:version/users", "list-users");
        assert_eq!(
            trie.lookup("/api/v2/users").map(|(h, _)| h),
            Some("list-users")
        );
        assert_eq!(trie.lookup("/api/v2"), None);
        assert_eq!(trie.lookup("/api/v2/posts"), None);
    }

    #[tokio::test]
    async fn fixture_server_answers_get_with_payload() {
        let server = FixtureServer::start("hello".to_owned()).await.unwrap();
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", server.port))
            .await
            .unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream
            .write_all(b"GET /payload HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        server.stop();
        assert!(response.contains("200 OK"), "{response}");
        assert!(response.contains("\"message\":\"hello\""), "{response}");
    }
}
