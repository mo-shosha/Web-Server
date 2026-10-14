# Rust Web Server

A simple HTTP web server built from scratch in Rust while learning the concepts covered in *The Rust Programming Language*, especially Chapter 20: **Building a Multithreaded Web Server**.

The project started with a basic TCP listener and gradually evolved into a web server that supports routing, HTTP responses, concurrent request handling, and a fixed-size thread pool.

## 1. Project Goals

The goal was not just to make a server work, but to understand how it works internally:

- How a TCP server accepts client connections.
- How HTTP requests and responses are structured.
- How to read incoming requests and send responses.
- How to implement basic routing.
- Why blocking operations can affect server responsiveness.
- How threads enable concurrent request handling.
- How a thread pool manages a fixed number of worker threads.
- How Rust ownership, closures, channels, smart pointers, and `Drop` work together in a real project.

## 2. Development Journey

### Stage 1: Creating the Rust Project

The project was created using Cargo:

```bash
cargo new rust_web_server
cd rust_web_server
cargo run
```

Initially, the application contained the default Rust program:

```rust
fn main() {
    println!("Hello, world!");
}
```

This confirmed that the project was configured correctly and ready for development.

### Stage 2: Starting a TCP Listener

**Problem:** The application needed to accept connections from web browsers.

**Solution:** We used `TcpListener` to bind the server to a local address.

```rust
let listener = TcpListener::bind("127.0.0.1:7878")
    .expect("Failed to bind to address");
```

The server then started listening for incoming connections.

At this stage, the server could accept TCP connections, but it did not yet process HTTP requests or send meaningful responses.

### Stage 3: Accepting Client Connections

**Problem:** Opening the server URL in a browser established a connection, but the server needed to handle incoming connections continuously.

**Solution:** We used `listener.incoming()` to iterate over incoming TCP connections.

```rust
for stream in listener.incoming() {
    let stream = stream.expect("Failed to establish connection");

    println!("Connection established!");
}
```

Now, the server could detect incoming connections.

However, it still needed to read the request sent by the browser.

### Stage 4: Reading HTTP Requests

**Problem:** Accepting a TCP connection does not automatically provide a parsed HTTP request.

**Solution:** We used `TcpStream::read()` to read bytes sent by the client.

```rust
let mut buffer = [0; 1024];

let bytes_read = stream.read(&mut buffer)?;

let request = String::from_utf8_lossy(&buffer[..bytes_read]);

println!("{request}");
```

The browser sent a request similar to:

```http
GET / HTTP/1.1
Host: 127.0.0.1:7878
Connection: keep-alive
```

This helped us understand that HTTP requests are transmitted as data over a TCP connection.

At this stage, the server could inspect requests, but it still needed to respond using the correct HTTP format.

### Stage 5: Sending an HTTP Response

**Problem:** The browser expects a valid HTTP response. Simply reading the request or printing a message in the terminal is not enough.

**Solution:** We constructed an HTTP response containing a status line, headers, a blank line, and a response body.

```rust
let response = "HTTP/1.1 200 OK\r\n\
Content-Length: 13\r\n\
Connection: close\r\n\
\r\n\
Hello, world!";

stream.write_all(response.as_bytes())?;
```

The response was divided into four parts:

1. **Status line:** `HTTP/1.1 200 OK`
2. **Headers:** Such as `Content-Length`
3. **Blank line:** Separates headers from the body.
4. **Body:** The content displayed by the browser.

The server could now return a valid response that the browser displayed.

### Stage 6: Adding Basic Routing

**Problem:** The server returned the same content regardless of the requested URL.

**Solution:** We extracted the first line of the HTTP request and used it to determine which response to send.

We introduced three routes:

| Request | Result |
|---|---|
| `GET / HTTP/1.1` | Returns the home page |
| `GET /sleep HTTP/1.1` | Waits five seconds, then returns a page |
| Any other request | Returns `404 Not Found` |

We also generated the response dynamically using `format!`, allowing the status and response body to change depending on the requested route.

The `Content-Length` header was calculated from the body length.

**Result:** The server could now handle different routes and return different HTML pages.

### Stage 7: Discovering the Blocking Problem

**Problem:** The `/sleep` route intentionally waited five seconds using:

```rust
thread::sleep(Duration::from_secs(5));
```

Because the server processed connections sequentially, a request to `/sleep` blocked the main request-handling loop.

If another browser tab requested `/`, that request had to wait until the previous request finished.

This demonstrated an important limitation of a single-threaded, blocking server.

**Solution attempt:** We created a new thread for each connection:

```rust
thread::spawn(move || {
    handle_connection(stream);
});
```

The request-handling logic was moved into a separate `handle_connection` function.

**Result:** Requests could be handled concurrently. A slow request no longer automatically blocked every other request.

**New problem:** Creating an unlimited number of threads could consume too many system resources if the server received many connections.

### Stage 8: Introducing a Thread Pool

**Problem:** Spawning a new thread for every incoming connection does not provide a fixed limit on the number of worker threads.

**Solution:** We implemented a fixed-size `ThreadPool` containing four workers.

```rust
let pool = ThreadPool::new(4);
```

Instead of creating a thread for every connection, the server submits each connection-handling task to the pool:

```rust
pool.execute(move || {
    handle_connection(stream);
});
```

The thread pool is implemented in `src/lib.rs`, while the HTTP server remains in `src/main.rs`.

This separates task management from HTTP request handling.

### Stage 9: Implementing the Thread Pool Internals

The thread pool uses several Rust concepts to coordinate work safely.

#### 9.1 Jobs and Closures

A job is represented by:

```rust
type Job = Box<dyn FnOnce() + Send + 'static>;
```

This allows the pool to accept different closures and store them behind a common type.

- `FnOnce` means a job can consume captured values and execute once.
- `Send` allows the job to be transferred safely between threads.
- `'static` ensures the job does not contain non-static borrowed references that could become invalid while a worker executes it.
- `Box` stores the closure behind a pointer with a known size, allowing different closure types to be stored in the same job representation.

#### 9.2 Communicating Through a Channel

We use Rust's multi-producer, single-consumer channel API:

```rust
let (sender, receiver) = mpsc::channel::<Message>();
```

The main thread sends messages to the workers. Each message contains either a new job or a termination instruction.

```rust
enum Message {
    NewJob(Job),
    Terminate,
}
```

This provides a communication mechanism between the thread pool and its worker threads.

#### 9.3 Sharing the Receiver Safely

All workers need access to the same receiver. We share it using:

```rust
let receiver = Arc::new(Mutex::new(receiver));
```

- `Arc<T>` allows multiple threads to share ownership.
- `Mutex<T>` ensures that only one worker at a time accesses the receiver.

Each worker locks the receiver to wait for a message. After receiving it, the worker releases the lock before executing the job.

This is important: the mutex protects access to the shared receiver, not the execution of the job itself. Different workers can execute different jobs concurrently.

#### 9.4 Worker Lifecycle

Each worker runs a loop that waits for messages.

When it receives `Message::NewJob(job)`, it executes the closure.

When it receives `Message::Terminate`, it exits the loop.

This allows the workers to remain alive and process multiple tasks instead of creating and destroying a thread for every request.

#### 9.5 Graceful Shutdown with `Drop`

The `ThreadPool` implements Rust's `Drop` trait.

When the pool is dropped, it sends termination messages to the workers and then joins their thread handles.

Calling `join()` waits for each worker thread to finish, helping the program shut down in an orderly manner rather than simply abandoning the worker threads.

## 3. Final Architecture

The request flow is now:

```text
Browser
   |
   | HTTP Request
   v
TcpListener
   |
   | Accept TCP connection
   v
ThreadPool::execute()
   |
   | Send NewJob through channel
   v
Available Worker
   |
   | Execute handle_connection(stream)
   v
Read HTTP Request
   |
   | Inspect request line
   v
Route Matching
   |
   +---- GET / -------> Home Page
   |
   +---- GET /sleep --> Wait 5 Seconds
   |
   +---- Other -------> 404 Not Found
   |
   v
Build HTTP Response
   |
   | Write response to TCP stream
   v
Browser Displays Result
```

## 4. How the Final Thread Pool Works

The final implementation uses four worker threads.

When a request arrives:

1. The main thread accepts the TCP connection.
2. It creates a closure that calls `handle_connection(stream)`.
3. `ThreadPool::execute()` wraps the closure in a `Job` and sends it through the channel.
4. An available worker receives the job.
5. The worker executes the closure and processes the HTTP request.
6. After finishing, the worker returns to waiting for another job.

If all four workers are busy, additional jobs remain queued in the channel until a worker becomes available.

The number of worker threads remains fixed at four, while the number of queued jobs can grow because the current channel is unbounded.

This provides more controlled resource usage than creating a new thread for every request, although a production server would also need limits and overload handling.

## 5. Project Structure

```text
rust_web_server/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── main.rs
```

- `main.rs`: Starts the server, accepts connections, submits jobs to the pool, reads requests, applies routing, and writes responses.
- `lib.rs`: Implements `ThreadPool`, `Worker`, the job type, the message protocol, and shutdown behavior.
- `Cargo.toml`: Contains the package metadata and build configuration.

## 6. How to Run the Server

From the project directory:

```bash
cargo run
```

The server listens at:

```text
http://127.0.0.1:7878
```

Available routes:

- Home page: `http://127.0.0.1:7878/`
- Slow route: `http://127.0.0.1:7878/sleep`
- Not found: `http://127.0.0.1:7878/anything-else`

To test concurrency, open `/sleep` in one browser tab and `/` in another. The home page should respond without waiting for the five-second sleep, provided a worker is available.

To check the code and tests:

```bash
cargo check
cargo test
```

## 7. Rust Concepts Practiced

This project combines several concepts from the Rust book:

- **TCP networking:** `TcpListener` and `TcpStream`
- **I/O:** Reading and writing bytes with `Read` and `Write`
- **HTTP basics:** Request lines, status codes, headers, and response bodies
- **Routing:** Matching requests to handlers
- **Concurrency:** Spawning and coordinating threads
- **Closures:** Representing tasks as executable values
- **Ownership and `move`:** Transferring the TCP stream into a worker job
- **Trait bounds:** `FnOnce`, `Send`, and `'static`
- **Channels:** Sending jobs and shutdown messages
- **Smart pointers:** Sharing ownership with `Arc`
- **Synchronization:** Protecting the receiver with `Mutex`
- **Error handling:** Using `Result`, `match`, and `expect`
- **`Drop`:** Coordinating graceful thread-pool shutdown

## 8. Current Limitations and Future Improvements

This project is an educational HTTP server, not a production-ready web server.

Potential improvements include:

- Parse HTTP requests more robustly instead of relying on exact request-line strings.
- Handle partial reads and larger requests correctly.
- Support additional HTTP methods and routes.
- Serve static files and implement proper MIME types.
- Add automated tests for routing, HTTP responses, and the thread pool.
- Limit the number of queued jobs to protect against overload.
- Improve shutdown behavior and handle worker panics more carefully.
- Add logging, configuration, and production-grade error handling.
- Consider a mature HTTP framework or an asynchronous runtime for more advanced workloads.

## Conclusion

The project evolved from a basic TCP listener into a multithreaded web server with routing and a fixed-size thread pool.

The most important lesson was understanding **why each change was needed**: first the server had to accept connections, then read requests, then respond using HTTP, then support multiple routes, then overcome blocking request handling, and finally control concurrency through a thread pool.

Rather than treating Rust concurrency as an isolated topic, this project applies ownership, closures, channels, synchronization, and thread management to a practical networking problem.
