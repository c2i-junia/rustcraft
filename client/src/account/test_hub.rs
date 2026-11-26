//! A stand-in for the hub in tests: a thread on `127.0.0.1` that answers the requests it gets
//! with canned HTTP/1.1 responses, in order or by route, and keeps what it was asked.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// What a request said.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    /// The headers, names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// The answer to one request: a status and a JSON body.
#[derive(Clone)]
pub struct Canned {
    pub status: u16,
    pub body: String,
}

impl Canned {
    pub fn ok(body: &str) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
        }
    }

    pub fn error(status: u16, code: &str, message: &str) -> Self {
        Self {
            status,
            body: format!(r#"{{"error":"{code}","message":"{message}"}}"#),
        }
    }

    /// An answer without a body, such as `204 No Content`.
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            body: String::new(),
        }
    }
}

/// The answers of a route of `TestHub::routes`: a method, a path, and the answers to its
/// requests in order, the last one repeated.
pub type Route = (&'static str, String, Vec<Canned>);

pub struct TestHub {
    /// `http://127.0.0.1:<port>`
    pub url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl TestHub {
    /// Answers the first request with `answers[0]`, the second with `answers[1]`, and so on,
    /// then stops.
    pub fn start(answers: Vec<Canned>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || {
            for answer in answers {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                if let Some(request) = read_request(&mut stream) {
                    seen.lock().unwrap().push(request);
                }
                answer_with(&mut stream, &answer);
            }
        });
        Self { url, requests }
    }

    /// Answers each request by its method and path, with the next answer of its route, the last
    /// one once they were all given; a request of no route gets a 404. Runs until the process
    /// ends.
    pub fn routes(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || {
            let mut routes: Vec<(&str, String, Vec<Canned>, usize)> = routes
                .into_iter()
                .map(|(method, path, answers)| (method, path, answers, 0))
                .collect();
            let not_found = Canned::error(404, "not_found", "No such route.");
            while let Ok((mut stream, _)) = listener.accept() {
                let Some(request) = read_request(&mut stream) else {
                    continue;
                };
                let answer = routes
                    .iter_mut()
                    .find(|(method, path, answers, _)| {
                        *method == request.method && *path == request.path && !answers.is_empty()
                    })
                    .map_or(not_found.clone(), |(_, _, answers, next)| {
                        let answer = answers[(*next).min(answers.len() - 1)].clone();
                        *next += 1;
                        answer
                    });
                seen.lock().unwrap().push(request);
                answer_with(&mut stream, &answer);
            }
        });
        Self { url, requests }
    }

    /// The requests received so far.
    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

fn answer_with(stream: &mut TcpStream, answer: &Canned) {
    let head = format!(
        "HTTP/1.1 {} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.status,
        answer.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(answer.body.as_bytes());
}

fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        if let Some(at) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let length = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Some(Recorded {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}
