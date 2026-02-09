# std.fs

The `std.fs` module provides file system operations. Many operations can raise `fs.FileError`.

```
import std.fs
```

## Reading and Writing Files

### read_all / write_all

The simplest way to work with files:

```
import std.fs

fn main() {
    fs.write_all("hello.txt", "hello, world")

    let content = fs.read_all("hello.txt") catch ""
    print(content)      // "hello, world"
}
```

### append_all

```
fs.append_all("log.txt", "new line\n")
```

## File Objects

For more control, open a file and work with it directly:

```
import std.fs

fn main() {
    let f = fs.open_write("data.txt") catch err { return }
    f.write("hello")
    f.close() catch err {}

    let f2 = fs.open_read("data.txt") catch err { return }
    let data = f2.read(1024)
    print(data)
    f2.close() catch err {}
}
```

### Opening Files

```
fs.open_read(path: string) fs.File       // Open for reading
fs.open_write(path: string) fs.File      // Open for writing (creates/truncates)
fs.open_append(path: string) fs.File     // Open for appending
```

### File Methods

```
file.read(max_bytes: int) string    // Read up to max_bytes
file.write(data: string) int        // Write data, returns bytes written
file.seek(offset: int, whence: int) int  // Seek to position
file.close()                         // Close the file
```

## File System Operations

### Checking Paths

```
fs.exists(path: string) bool
fs.is_file(path: string) bool
fs.is_dir(path: string) bool
fs.file_size(path: string) int
```

### Manipulating Files

```
fs.remove(path: string)              // Delete a file
fs.rename(from: string, to: string)  // Rename/move
fs.copy(from: string, to: string)    // Copy a file
```

### Directories

```
fs.mkdir(path: string)               // Create a directory
fs.rmdir(path: string)               // Remove an empty directory
fs.list_dir(path: string) [string]   // List directory contents
fs.temp_dir() string                 // Get the system temp directory
```

## Error Handling

Most `fs` operations can raise `fs.FileError`. Use `catch` or `!` to handle them:

```
import std.fs

fn main() {
    let content = fs.read_all("missing.txt") catch "file not found"
    print(content)
}
```
