# std.socket

Low-level TCP socket operations. For higher-level HTTP, use `std.http` or `std.net`.

```
import std.socket
```

## Functions

### create

```
socket.create(domain: int, sock_type: int, protocol: int) int
```

Creates a socket and returns a file descriptor. Use domain `2` (AF_INET), sock_type `1` (SOCK_STREAM), protocol `0`.

### bind

```
socket.bind(fd: int, host: string, port: int) int
```

Binds a socket to an address and port.

### listen

```
socket.listen(fd: int, backlog: int) int
```

Marks a socket as listening for incoming connections.

### accept

```
socket.accept(fd: int) int
```

Accepts a connection and returns a new file descriptor for the client.

### connect

```
socket.connect(fd: int, host: string, port: int) int
```

Connects to a remote host.

### read

```
socket.read(fd: int, max_bytes: int) string
```

Reads up to `max_bytes` from a socket.

### write

```
socket.write(fd: int, data: string) int
```

Writes data to a socket. Returns number of bytes written.

### read_bytes / write_bytes

```
socket.read_bytes(fd: int, max_bytes: int) bytes
socket.write_bytes(fd: int, data: bytes) int
```

Binary-safe variants of `read` and `write`. `read_bytes` returns empty bytes on EOF or error.

### set_read_timeout / read_timed_out

```
socket.set_read_timeout(fd: int, ms: int) int
socket.read_timed_out() int
```

`set_read_timeout` sets a read deadline (`SO_RCVTIMEO`): later reads block at most `ms` milliseconds, and `ms <= 0` clears it. After a read comes back empty, `read_timed_out()` returns `1` if the deadline elapsed and `0` for EOF or a connection error.

### close

```
socket.close(fd: int) int
```

Closes a socket.

### set_reuseaddr

```
socket.set_reuseaddr(fd: int) int
```

Sets the `SO_REUSEADDR` option on a socket.

### get_port

```
socket.get_port(fd: int) int
```

Returns the port number a socket is bound to.

## Example

```
import std.socket

fn main() {
    let fd = socket.create(2, 1, 0)
    socket.set_reuseaddr(fd)
    socket.bind(fd, "0.0.0.0", 8080)
    socket.listen(fd, 128)
    print("Listening on :8080")
    let client = socket.accept(fd)
    let data = socket.read(client, 4096)
    print(f"Received: {data}")
    socket.close(client)
    socket.close(fd)
}
```
