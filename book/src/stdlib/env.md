# std.env

Environment variable access.

```
import std.env
```

## Functions

### get

```
env.get(name: string) string
```

Returns the value of the environment variable, or an empty string if not set.

### get_or

```
env.get_or(name: string, default_val: string) string
```

Returns the value of the environment variable, or `default_val` if not set.

```
let port = env.get_or("PORT", "8080")
```

### set

```
env.set(name: string, value: string)
```

Sets an environment variable.

### exists

```
env.exists(name: string) bool
```

Returns `true` if the environment variable is set.

### list_names

```
env.list_names() [string]
```

Returns an array of all environment variable names.

### remove

```
env.remove(name: string) bool
```

Removes an environment variable. Returns `true` if it existed.

## Example

```
import std.env

fn main() {
    let host = env.get_or("DATABASE_HOST", "localhost")
    let port = env.get_or("DATABASE_PORT", "5432")
    print("Connecting to {host}:{port}")
}
```
