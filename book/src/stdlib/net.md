# std.net

TCP networking: server and client sockets.

```
import std.net
```

`TcpListener` and `TcpConnection` are **objects** — entities owning OS resources (a bound port, a live socket). They compare by identity, are *shared* (not deep-copied) across `spawn` — handing a listener to a task never forks the descriptor — and their methods serialize per instance, so concurrent users of one connection cannot interleave mid-read or mid-write. See [Objects and Entities](../whats-different/objects.md).

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
