# std.rpc

HTTP-based RPC client for cross-stage method calls. Used internally by the compiler-generated RPC stubs.

```
import std.rpc
```

## Functions

### http_post

```
rpc.http_post(url: string, body: string) string
```

Sends an HTTP POST request with a 5-second timeout. Returns the response body.

### http_post_with_timeout

```
rpc.http_post_with_timeout(url: string, body: string, timeout_ms: int) string
```

Sends an HTTP POST request with a custom timeout in milliseconds.
