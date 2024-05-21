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

#[derive(Debug)]
struct RequestState {
    conn_stream: TcpStream,
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
        "/",
        PathProperties {
            handler: handle_path_status as fn(AppState) -> (),
            method_conent_map: HashMap::from([("GET", "application/json")]),
        },
    )]));

    // setup server
    let server = TcpListener::bind("127.0.0.1:3000").expect("Could not bind to address");

    // handle incoming connections
    let thread_pool = thread::spawn(move || {
        for stream in server.incoming() {
            if let Some(stream) = stream.ok() {
                let mut state = AppState {
                    status_code_response_map: status_code_response_map.clone(),
                    routes: routes.clone(),
                    request_state: RequestState {
                        conn_stream: stream,
                        request_buffer: [32; 1024],
                    },
                };
                thread::spawn(move || {
                    state
                        .request_state
                        .conn_stream
                        .read(&mut state.request_state.request_buffer)
                        .unwrap();

                    // ["POST / HTTP/1.1", "Host: 127.0.0.1:3000", "User-Agent: curl/7.88.1", "Accept: */*", "Content-Length: 17", "Content-Type: application/x-www-form-urlencoded", "", "{'field': 'data'}"]
                    let request = String::from_utf8_lossy(&state.request_state.request_buffer[..]);
                    let request_lines: Vec<&str> = request.trim().split("\r\n").collect();

                    // parse info
                    let request_info = request_lines[0].split_whitespace().collect::<Vec<&str>>();
                    if request_info.len() != 3 {
                        return_response(state, 400, None, None);
                        return;
                    }

                    // parse headers.
                    let headers = request_lines
                        .iter()
                        .skip(1)
                        .take_while(|line| !line.is_empty())
                        .collect::<Vec<&&str>>()
                        .pop();
                    if headers.is_none() {
                        return_response(state, 400, None, None);
                        return;
                    }

                    // parse body.
                    // let body = request_lines.last();

                    // handle path
                    let path_properties = match state.routes.get(request_info[1]) {
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
                        return_response(state, 405, None, None);
                        return;
                    }

                    (path_properties.handler)(state);
                });
            }
        }
    });
    thread_pool.join().unwrap();
}

fn return_response(
    mut state: AppState,
    code: u16,
    headers: Option<HashMap<&str, &str>>,
    body: Option<Value>,
) {
    let response_body = body.unwrap_or(json!({})).to_string();
    let response_body_len = response_body.len().to_string();
    let response = format!(
        "HTTP/1.1 {} {}\n{}\r\n\r\n{}",
        code,
        state.status_code_response_map.get(&code).unwrap(),
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
    state
        .request_state
        .conn_stream
        .write(response.as_bytes())
        .unwrap();
}

fn handle_path_status(state: AppState) {
    return_response(state, 200, None, None);
}

fn handle_404(state: AppState) {
    return_response(
        state,
        404,
        None,
        Some(json!({"error": "The requested resource was not found."})),
    );
}
