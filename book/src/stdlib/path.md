# std.path

File path manipulation utilities. Pure string operations -- no filesystem access.

```
import std.path
```

## Functions

### join

```
path.join(base: string, rel: string) string
```

Joins two path segments with `/`.

```
path.join("/home", "user")    // "/home/user"
path.join("/home/", "user")   // "/home/user"
```

### basename

```
path.basename(p: string) string
```

Returns the last component of the path.

```
path.basename("/home/user/file.txt")  // "file.txt"
```

### dirname

```
path.dirname(p: string) string
```

Returns the directory portion of the path.

```
path.dirname("/home/user/file.txt")  // "/home/user"
```

### ext

```
path.ext(p: string) string
```

Returns the file extension including the dot, or empty string if none.

```
path.ext("file.txt")     // ".txt"
path.ext("Makefile")     // ""
```

### split_ext

```
path.split_ext(p: string) string
```

Returns the filename without the extension.

```
path.split_ext("file.txt")  // "file"
```

### is_absolute

```
path.is_absolute(p: string) bool
```

Returns `true` if the path starts with `/`.

### has_trailing_slash

```
path.has_trailing_slash(p: string) bool
```

Returns `true` if the path ends with `/`.

### normalize

```
path.normalize(p: string) string
```

Normalizes a path by resolving `.` and `..` components and removing duplicate slashes.

```
path.normalize("/home/user/../admin/./docs")  // "/home/admin/docs"
```

## Example

```
import std.path

fn main() {
    let p = "/home/user/documents/report.pdf"
    print("Dir: {path.dirname(p)}")
    print("File: {path.basename(p)}")
    print("Ext: {path.ext(p)}")
}
```
