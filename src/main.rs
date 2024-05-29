use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::hash_map::HashMap;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

const CACHE_CLEANUP_INTERVAL: u64 = 60; // seconds
const CACHE_RETAIN_INTERVAL: i64 = 60; // seconds

#[derive(Debug)]
struct PathProperties {
    handler: fn(RequestState) -> (),
    method_conent_map: HashMap<&'static str, &'static str>,
}

#[derive(Debug)]
struct CacheResponse {
    response: String,
    time: chrono::DateTime<chrono::Utc>,
}
struct RequestState {
    code_map: Arc<HashMap<u16, &'static str>>,
    routes: Arc<HashMap<&'static str, PathProperties>>,
    conn_stream: TcpStream,
    request_buffer: [u8; 1024],
    request_cache: Arc<Mutex<HashMap<Vec<u8>, CacheResponse>>>,
    request_hash: Option<Vec<u8>>,
}

impl RequestState {
    fn new(
        code_map: Arc<HashMap<u16, &'static str>>,
        routes: Arc<HashMap<&'static str, PathProperties>>,
        cache: Arc<Mutex<HashMap<Vec<u8>, CacheResponse>>>,
        stream: TcpStream,
    ) -> RequestState {
        RequestState {
            code_map: code_map,
            routes: routes,
            request_cache: cache,
            conn_stream: stream,
            request_buffer: [32; 1024],
            request_hash: None,
        }
    }
}

fn main() {
    //  initialize server defaults
    let mut jh_vec = Vec::<std::thread::JoinHandle<()>>::new();
    let code_map = Arc::new(HashMap::from([
        (200, "OK"),
        (201, "Created"),
        (204, "No Content"),
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (405, "Method Not Allowed"),
        (500, "Internal Server Error"),
    ]));
    let routes = Arc::new(HashMap::from([(
        "/",
        PathProperties {
            handler: handle_path_status as fn(RequestState) -> (),
            method_conent_map: HashMap::from([("GET", "application/json")]),
        },
    )]));
    let request_cache = Arc::new(Mutex::new(HashMap::new()));

    // create server
    let server = TcpListener::bind("127.0.0.1:3000").expect("Could not bind to address");

    // handle connections
    let request_cache_clone = Arc::clone(&request_cache);
    let connection_pool = thread::spawn(move || {
        for stream_result in server.incoming() {
            // if the stream is valid, spawn a new thread to handle the request
            match stream_result {
                Ok(stream) => {
                    let mut request_state = RequestState::new(
                        code_map.clone(),
                        routes.clone(),
                        request_cache_clone.clone(),
                        stream,
                    );
                    thread::spawn(move || {
                        // read request to buffer
                        if request_state
                            .conn_stream
                            .read(&mut request_state.request_buffer)
                            .is_err()
                        {
                            return_response(request_state, 500, None, None);
                            return;
                        }

                        // resolve cache
                        let request_hash = Sha256::digest(&request_state.request_buffer).to_vec();
                        request_state.request_hash = Some(request_hash.clone());
                        if let Ok(cache) = request_state.request_cache.lock() {
                            if cache.contains_key(&request_hash) {
                                let cached_response = cache.get(&request_hash).unwrap();
                                request_state
                                    .conn_stream
                                    .write(cached_response.response.as_bytes())
                                    .unwrap();
                                return ();
                            }
                        };

                        // parse request.
                        let request = String::from_utf8_lossy(&request_state.request_buffer[..]);
                        let request_lines: Vec<&str> = request.trim().split("\r\n").collect();
                        let request_info =
                            request_lines[0].split_whitespace().collect::<Vec<&str>>();
                        if request_info.len() != 3 {
                            return_response(request_state, 400, None, None);
                            return;
                        }
                        // build headers.
                        let _headers: HashMap<&str, &str> = request_lines
                            .iter()
                            .skip(1)
                            .take_while(|line| !line.is_empty())
                            .map(|line| {
                                let mut parts = line.split(": ");
                                (parts.next().unwrap(), parts.next().unwrap())
                            })
                            .collect::<HashMap<&str, &str>>();
                        // parse body.
                        // let body = request_lines.last();

                        // if route isn't defined, return 404.
                        let path_properties = match request_state.routes.get(request_info[1]) {
                            Some(properties) => properties,
                            None => {
                                handle_404(request_state);
                                return;
                            }
                        };

                        // if method isn't defined, return 405.
                        if !path_properties
                            .method_conent_map
                            .contains_key(request_info[0])
                        {
                            return_response(request_state, 405, None, None);
                            return;
                        }

                        (path_properties.handler)(request_state);
                    });
                }
                Err(_) => {
                    continue;
                }
            }
        }
    });
    jh_vec.push(connection_pool);

    let request_cache_clone = Arc::clone(&request_cache);
    let cache_cleaner = thread::spawn(move || loop {
        if let Ok(mut cache) = request_cache_clone.lock() {
            let now = chrono::Utc::now();
            cache.retain(|_, value| {
                let duration = now.signed_duration_since(value.time);
                duration.num_seconds() < CACHE_RETAIN_INTERVAL
            });
        }
        std::thread::sleep(std::time::Duration::from_secs(CACHE_CLEANUP_INTERVAL));
    });
    jh_vec.push(cache_cleaner);

    for jh in jh_vec {
        jh.join().unwrap();
    }
}

fn return_response(
    mut state: RequestState,
    code: u16,
    headers: Option<HashMap<&str, &str>>,
    body: Option<Value>,
) {
    let response_body = body.unwrap_or(json!({})).to_string();
    let response_body_len = response_body.len().to_string();
    let response = format!(
        "HTTP/1.1 {} {}\n{}\r\n\r\n{}",
        code,
        state.code_map.get(&code).unwrap(),
        {
            let mut headers = headers.unwrap_or(HashMap::new());
            if !headers.contains_key("Content-Type") {
                headers.insert("Content-Type", "application/json");
            }
            if !headers.contains_key("Content-Length") {
                headers.insert("Content-Length", &response_body_len);
            }
            if !headers.contains_key("Connection") {
                headers.insert("Connection", "close");
            }
            if !headers.contains_key("Server") {
                headers.insert("Server", "Hotpocket :3");
            }
            if !headers.contains_key("X-Frame-Options") {
                headers.insert("X-Frame-Options", "SAMEORIGIN");
            }
            headers
                .iter()
                .map(|(key, value)| format!("{}: {}\r\n", key, value))
                .collect::<String>()
                .trim()
        },
        response_body
    );
    if let Some(request_hash) = state.request_hash {
        let mut cache = state.request_cache.lock().unwrap();
        cache.insert(
            request_hash,
            CacheResponse {
                response: response.clone(),
                time: chrono::Utc::now(),
            },
        );
    }
    state.conn_stream.write(response.as_bytes()).unwrap();
}

fn handle_path_status(state: RequestState) {
    return_response(state, 200, None, None);
}

fn handle_404(state: RequestState) {
    return_response(
        state,
        404,
        None,
        Some(json!({"error": "The requested resource was not found."})),
    );
}
