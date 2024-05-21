use std::collections::hash_map::HashMap;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use serde_json::{json, Value};

#[derive(Debug)]
struct PathProperties {
    handler: fn(AppState) -> (),
    method_conent_map: HashMap<&'static str, &'static str>,
}
type PathHandlerMap = Arc<HashMap<&'static str, PathProperties>>;

#[derive(Debug, Clone)]
struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}
#[derive(Debug)]
struct RequestState {
    stream: TcpStream,
    request: Option<Request>,
    request_buffer: [u8; 1024],
}
struct AppState {
    status_code_response_map: HashMap<u16, &'static str>,
    routes: PathHandlerMap,
    request_state: RequestState,
}

fn main() {
    // constants initialization
    let status_code_response_map: HashMap<u16, &'static str> = HashMap::from([
        (200, "OK"),
        (201, "Created"),
        (204, "No Content"),
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (405, "Method Not Allowed"),
        (500, "Internal Server Error"),
    ]);
    let routes = Arc::new(HashMap::from([(
        "/signup",
        PathProperties {
            handler: handle_path_status as fn(AppState) -> (),
            method_conent_map: HashMap::from([("POST", "application/json")]),
        },
    )]));

    // setup server
    let server = TcpListener::bind("127.0.0.1:3000").expect("Could not bind to address");

    // handle incoming connections
    let thread_pool = thread::spawn(move || {
        for stream in server.incoming() {
            let mut state = AppState {
                status_code_response_map: status_code_response_map.clone(),
                routes: routes.clone(),
                request_state: RequestState {
                    stream: stream.expect("Could not establish connection"),
                    request: None,
                    request_buffer: [32; 1024],
                },
            };
            thread::spawn(move || {
                state
                    .request_state
                    .stream
                    .read(&mut state.request_state.request_buffer)
                    .unwrap();

                // ["POST / HTTP/1.1", "Host: 127.0.0.1:3000", "User-Agent: curl/7.88.1", "Accept: */*", "Content-Length: 17", "Content-Type: application/x-www-form-urlencoded", "", "{'field': 'data'}"]
                let request = String::from_utf8_lossy(&state.request_state.request_buffer[..]);
                let request_lines: Vec<&str> = request.trim().split("\r\n").collect();

                // parse request info.
                let request_info = request_lines[0].split_whitespace().collect::<Vec<&str>>();
                if request_info.len() != 3 {
                    return_response(state, 400, None);
                    return;
                }
                state.request_state.request = Some(Request {
                    method: request_info[0].to_string(),
                    path: request_info[1].to_string(),
                    headers: HashMap::new(),
                    body: None,
                });

                // parse headers.
                let mut header_iterator = request_lines.iter().skip(1);
                while let Some(line) = header_iterator.next() {
                    if line.is_empty() {
                        break;
                    }
                    let split_header: Vec<&str> = line.split(": ").collect();
                    state
                        .request_state
                        .request
                        .as_mut()
                        .unwrap()
                        .headers
                        .insert(split_header[0].to_string(), split_header[1].to_string());
                }

                // parse body.
                let body = request_lines.last().unwrap_or(&"");
                if !body.is_empty() {
                    state
                        .request_state
                        .request
                        .as_mut()
                        .unwrap()
                        .body
                        .replace(body.to_string());
                }

                // handle path
                let path = state.request_state.request.as_ref().unwrap().path.as_str();
                let path_properties = match state.routes.get(path) {
                    Some(properties) => properties,
                    None => {
                        handle_404(state);
                        return;
                    }
                };

                // handle methods that aren't allowed
                if !path_properties
                    .method_conent_map
                    .contains_key(request_info[0])
                {
                    return_response(state, 405, None);
                    return;
                }

                (path_properties.handler)(state);
            });
        }
    });
    thread_pool.join().unwrap();
}

fn return_response(mut state: AppState, status_code: u16, body: Option<Value>) {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\r\n{}",
        status_code,
        state.status_code_response_map.get(&status_code).unwrap(),
        body.unwrap_or(json!({})).to_string()
    );
    state
        .request_state
        .stream
        .write(response.as_bytes())
        .unwrap();
}

fn handle_path_status(state: AppState) {
    return_response(state, 200, None);
}

fn handle_404(state: AppState) {
    return_response(state, 404, None);
}
