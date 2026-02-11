# std.fs

File system operations. Most functions raise `fs.FileError` on failure.

```
import std.fs
```

## Errors

```
pub error FileError {
    message: string
}
```

## Quick Read/Write

### read_all / write_all / append_all

```
fs.read_all(path: string) string           // raises FileError
fs.write_all(path: string, data: string)   // raises FileError
fs.append_all(path: string, data: string)  // raises FileError
```

```
fs.write_all("hello.txt", "hello, world")!
let content = fs.read_all("hello.txt")!
fs.append_all("hello.txt", "\nmore data")!
```

## File Handle API

For streaming or partial reads/writes, open a `File` handle:

```
fs.open_read(path: string) File       // raises FileError
fs.open_write(path: string) File      // creates or truncates; raises FileError
fs.open_append(path: string) File     // raises FileError
```

### File Methods

```
file.read(max_bytes: int) string             // read up to max_bytes
file.write(data: string) int                 // write data, returns bytes written; raises FileError
file.seek(offset: int, whence: int) int      // seek, returns new position; raises FileError
file.close()                                  // raises FileError
```

Seek constants:

```
fs.SEEK_SET() int    // seek from beginning
fs.SEEK_CUR() int    // seek from current position
fs.SEEK_END() int    // seek from end
```

```
let f = fs.open_write("data.txt")!
f.write("hello")!
f.close()!

let f2 = fs.open_read("data.txt")!
let data = f2.read(1024)
f2.close()!
```

## Path Queries

```
fs.exists(path: string) bool
fs.is_file(path: string) bool
fs.is_dir(path: string) bool
fs.file_size(path: string) int     // raises FileError
```

## File Operations

```
fs.remove(path: string)                   // delete a file; raises FileError
fs.rename(from: string, to: string)       // rename or move; raises FileError
fs.copy(from: string, to: string)         // copy a file; raises FileError
```

## Directory Operations

```
fs.mkdir(path: string)                    // create directory; raises FileError
fs.rmdir(path: string)                    // remove empty directory; raises FileError
fs.list_dir(path: string) [string]        // list entries; raises FileError
fs.temp_dir() string                      // system temp directory
```

## Example: Log File

```
import std.fs

fn main() {
    let log_path = fs.temp_dir() + "/app.log"

    fs.write_all(log_path, "=== Log Start ===\n")!
    fs.append_all(log_path, "event: started\n")!
    fs.append_all(log_path, "event: processing\n")!

    let content = fs.read_all(log_path)!
    print(content)

    let size = fs.file_size(log_path)!
    print("Log size: {size} bytes")

    fs.remove(log_path)!
}
```
