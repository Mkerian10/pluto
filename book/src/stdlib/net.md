# std.net

TCP networking: server and client sockets.

```
import std.net
```

## TCP Server

### listen

```
net.listen(host: string, port: int) TcpListener
```

Binds a TCP server socket. Use port `0` for an OS-assigned port.

### TcpListener

| Method | Signature |
|--------|-----------|
| `accept` | `accept(self) TcpConnection` -- blocks until a client connects |
| `port` | `port(self) int` -- returns the bound port |
| `close` | `close(self) int` -- closes the listener |

```
let server = net.listen("127.0.0.1", 0)
print("Listening on port {server.port()}")

while true {
    let conn = server.accept()
    let data = conn.read(4096)
    conn.write("echo: {data}")
    conn.close()
}
```

## TCP Client

### connect

```
net.connect(host: string, port: int) TcpConnection
```

Opens a TCP connection to the given host and port.

### TcpConnection

| Method | Signature |
|--------|-----------|
| `read` | `read(self, max_bytes: int) string` -- reads up to max_bytes |
| `write` | `write(self, data: string) int` -- writes data, returns bytes written |
| `close` | `close(self) int` -- closes the connection |

```
let conn = net.connect("127.0.0.1", 8080)
conn.write("hello")
let response = conn.read(4096)
print(response)
conn.close()
```

## Example: Echo Server

```
import std.net

fn main() {
    let server = net.listen("127.0.0.1", 8080)
    print("Echo server on port {server.port()}")

    while true {
        let conn = server.accept()
        let msg = conn.read(4096)
        conn.write(msg)
        conn.close()
    }
}
```
