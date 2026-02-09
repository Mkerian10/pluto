# std.net

The `std.net` module provides TCP networking.

```
import std.net
```

## TCP Server

Use `net.listen()` to create a server:

```
import std.net

fn main() {
    let server = net.listen("127.0.0.1", 8080)
    let port = server.port()
    print("listening on port {port}")

    while true {
        let conn = server.accept()
        let data = conn.read(4096)
        print("received: {data}")
        conn.write("echo: {data}")
        conn.close()
    }
}
```

### TcpListener

```
net.listen(host: string, port: int) net.TcpListener
```

| Method | Description |
|--------|-------------|
| `accept(self) TcpConnection` | Wait for and accept a client connection |
| `port(self) int` | Get the port the server is listening on |
| `close(self) int` | Close the listener |

## TCP Client

Use `net.connect()` to connect to a server:

```
import std.net

fn main() {
    let conn = net.connect("127.0.0.1", 8080)
    conn.write("hello")
    let response = conn.read(4096)
    print(response)
    conn.close()
}
```

### TcpConnection

```
net.connect(host: string, port: int) net.TcpConnection
```

| Method | Description |
|--------|-------------|
| `read(self, max_bytes: int) string` | Read up to max_bytes from the connection |
| `write(self, data: string) int` | Write data to the connection |
| `close(self) int` | Close the connection |

## Example: HTTP Server

Here's a minimal HTTP server in Pluto:

```
import std.net
import std.fs
import std.strings

fn main() {
    let server = net.listen("127.0.0.1", 8080)
    print("listening on http://127.0.0.1:8080")

    while true {
        let conn = server.accept()
        let request = conn.read(4096)

        let body = "<h1>Hello from Pluto!</h1>"
        let len = "{body.len()}"
        let response = "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\n\r\n{body}"
        conn.write(response)
        conn.close()
    }
}
```
