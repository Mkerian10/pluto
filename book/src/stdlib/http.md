# std.http

HTTP/1.1 server with request parsing and response helpers.

```
import std.http
```

## Errors

```
pub error HttpError {
    message: string
}
```

Raised by `listen()` and `HttpConnection.read_request()`.

## Server

### listen

```
http.listen(host: string, port: int) HttpServer    // raises HttpError
```

Binds an HTTP server socket.

### HttpServer

| Method | Signature |
|--------|-----------|
| `accept` | `accept(self) HttpConnection` -- raises HttpError |
| `port` | `port(self) int` |
| `close` | `close(self)` |

### HttpConnection

| Method | Signature |
|--------|-----------|
| `read_request` | `read_request(self) Request` -- raises HttpError |
| `send_response` | `send_response(self, resp: Response)` |
| `close` | `close(self)` |

## Request

```
pub class Request {
    method: string        // "GET", "POST", etc.
    path: string          // "/hello", "/users/123"
    headers_raw: string   // raw header block
    body: string          // request body
}
```

| Method | Signature |
|--------|-----------|
| `header` | `header(self, name: string) string` -- case-insensitive lookup, returns `""` if missing |

## Response Helpers

Convenience constructors for common responses:

```
http.ok(body: string) Response                                      // 200, text/plain
http.ok_json(json_string: string) Response                          // 200, application/json
http.not_found() Response                                           // 404
http.bad_request() Response                                         // 400
http.response(status: int, status_text: string, body: string) Response   // custom status
```

### Response

```
pub class Response {
    status: int
    status_text: string
    headers_raw: string
    body: string
}
```

## Example: JSON API

```
import std.http
import std.json

fn handle(req: http.Request) http.Response {
    if req.path == "/hello" {
        let body = json.object()
        body.set("message", json.string("Hello, World!"))
        return http.ok_json(body.to_string())
    }

    if req.path == "/echo" {
        if req.method == "POST" {
            let data = json.parse(req.body) catch json.Json {
                tag: 0, int_val: 0, float_val: 0.0,
                string_val: "", children: [], keys: []
            }
            let resp = json.object()
            resp.set("echo", data)
            return http.ok_json(resp.to_string())
        }
        return http.bad_request()
    }

    return http.not_found()
}

fn main() {
    let server = http.listen("0.0.0.0", 8080)!
    print("Listening on :8080")

    while true {
        let conn = server.accept()!
        let req = conn.read_request() catch http.Request {
            method: "", path: "", headers_raw: "", body: ""
        }
        if req.method != "" {
            conn.send_response(handle(req))
        }
        conn.close()
    }
}
```
