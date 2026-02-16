# std.log

Structured logging with level filtering.

```
import std.log
```

## Log Levels

```
enum Level {
    Debug
    Info
    Warn
    Error
}
```

Output goes to stderr with timestamps in millisecond precision.

## Functions

### debug

```
log.debug(message: string)
```

Logs at DEBUG level.

### info

```
log.info(message: string)
```

Logs at INFO level.

### warn

```
log.warn(message: string)
```

Logs at WARN level.

### log_error

```
log.log_error(message: string)
```

Logs at ERROR level. Named `log_error` to avoid conflict with the `error` keyword.

### set_level

```
log.set_level(level: log.Level)
```

Sets the minimum log level. Messages below this level are silently dropped.

### get_level

```
log.get_level() log.Level
```

Returns the current minimum log level.

## Example

```
import std.log

fn main() {
    log.set_level(log.Level.Info)
    log.debug("this is suppressed")
    log.info("application started")
    log.warn("disk space low")
    log.log_error("connection failed")
}
```
