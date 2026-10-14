
use rust_web_server::ThreadPool;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878")
        .expect("Failed to bind to address");

    let pool = ThreadPool::new(4);

    println!("Server listening on http://127.0.0.1:7878");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("Failed to establish connection: {error}");
                continue;
            }
        };

        pool.execute(move || {
            handle_connection(stream);
        });
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    let bytes_read = match stream.read(&mut buffer) {
        Ok(0) => return,
        Ok(n) => n,
        Err(error) => {
            eprintln!("Failed to read request: {error}");
            return;
        }
    };

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next().unwrap_or("");

    println!("Request: {request_line}");

    let (status, body) = match request_line {
        "GET / HTTP/1.1" => (
            "200 OK",
            "<html><body><h1>Hello!</h1><p>Hi from Rust</p></body></html>",
        ),
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(5));

            (
                "200 OK",
                "<html><body><h1>Finished sleeping!</h1></body></html>",
            )
        }
        _ => (
            "404 Not Found",
            "<html><body><h1>404 - Not Found</h1></body></html>",
        ),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    if let Err(error) = stream.write_all(response.as_bytes()) {
        eprintln!("Failed to write response: {error}");
    }
}
