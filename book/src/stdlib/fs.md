# std.fs

File system operations: one-shot helpers for the common cases, an atomic durable replace, and a typestated file handle whose protocol — open, use, close — is checked by the compiler.

```
import std.fs
```

The design premise: the OS is the *authority* over the real file state, and a `File` value is *evidence* — a capability the authority minted at open, which you must eventually hand back. Leaking a handle, using it after close, writing through a read handle, or retrying a failed sync are not runtime errors in this module; they are programs that do not compile. See [Typestates](../whats-different/typestates.md) for the machinery.

## Errors

```
pub error NotFound { path: string }                                 // ENOENT
pub error FileError { path: string, code: int, message: string }    // any other OS failure
pub error CloseError { code: int, message: string }                 // close() failed; fd released regardless
pub error SyncError { path: string, code: int, message: string }    // one-shot durability failure
pub error Degraded { file: File<Write, Poisoned>, code: int, message: string }
```

The split is by what callers *do*, not by errno taxonomy. `NotFound` is the one branch-worthy case (the 404 shape, config defaulting) and gets its own type; everything else definite is a `FileError` carrying the path and the raw errno captured at the syscall site — no string matching. `CloseError`, `SyncError`, and `Degraded` are distinct so a generic handler cannot accidentally absorb a durability loss; `Degraded` is covered under [File Handles](#degradation-a-failed-write-or-sync-poisons-the-handle) below.

(`FileError.path` is `""` when raised by a handle method like `read` or `seek` — the handle carries only a descriptor, not the path it was opened from.)

## Quick Read/Write

### read_all / write_all / append_all

```
fs.read_all(path: string) string           // raises NotFound | FileError
fs.write_all(path: string, data: string)   // raises NotFound | FileError
fs.append_all(path: string, data: string)  // raises NotFound | FileError
```

```
fs.write_all("hello.txt", "hello, world")!
let content = fs.read_all("hello.txt")!
fs.append_all("hello.txt", "\nmore data")!
```

The descriptor lives and dies inside the call, so no handle value appears — and close errors are folded into the result: success means the bytes were written *and* the descriptor closed cleanly. (On NFS-class filesystems, close is where deferred write errors surface; these helpers do not swallow them.)

## Durable Replace

### replace_all

```
fs.replace_all(path: string, data: string)   // raises NotFound | FileError | SyncError
```

The atomic durable update, packaged: write a temp file in the *same* directory, sync it to full durability, rename it over `path`, then fsync the parent directory (the name→inode mapping is the directory's dirty page, not the file's). Readers see either the old contents or the new contents, never a torn mix, and a crash after success cannot roll the update back.

The error contract is two-sided — the type tells you what survived:

- **`FileError`** (or `NotFound`) ⇒ the old file is **intact**. The failure happened before the rename; the temp file was cleaned up.
- **`SyncError`** ⇒ the rename **landed**, but its durability is unwarranted — the directory fsync failed, so a crash could still lose it.

```
fs.replace_all(config, "retries = 5\n") catch e: fs.SyncError {
    // New config IS in place, durability unwarranted. Treat as failed; alert.
    print(f"replace landed without durability warrant: {e.message}")
    return
} catch e: fs.FileError {
    // Old config intact; temp cleaned up. Safe to retry or report.
    print(f"replace failed, old config intact: {e.message}")
    return
}
```

The four steps are packaged because this is the only safe traversal, and intermediate failures must clean up the temp file.

### sync_dir

```
fs.sync_dir(path: string)   // raises SyncError
```

fsync a directory — required for crash-safe rename/create/remove compositions you build yourself. One-shot by nature; there is no directory handle.

## File Handles

For streaming or partial reads and writes, open a handle:

```
fs.open_read(path: string) File<Read, Open>      // raises NotFound | FileError
fs.open_write(path: string) File<Write, Open>    // O_WRONLY|O_CREAT|O_TRUNC
fs.open_append(path: string) File<Write, Open>   // O_WRONLY|O_CREAT|O_APPEND
```

`File<M, S>` is a **typestated class** — a value carrying evidence, not an [entity](../whats-different/objects.md). The mode `M` (`Read` or `Write`) is fixed at open; the state `S` (`Open`, `Poisoned`, or `Closed`) changes only through consuming transitions. Append-ness lives in the descriptor (`O_APPEND`), not the type, so `open_append` also yields a `File<Write, Open>`.

Three guarantees ride on the type:

- **Leaks are compile errors.** `Open` and `Poisoned` are `must_release` states: dropping a live handle, capturing it in a closure or `spawn`, or storing it in a field is rejected at compile time, with a diagnostic naming the obligation and the transitions out.
- **Method existence is per-state.** `read` exists only `where M == Read, S == Open`; `write`, `sync`, and `sync_data` only `where M == Write, S == Open`. Use-after-close, double-close, and write-on-a-read-handle are not runtime errors — the methods do not exist on those types.
- **Transitions consume.** `close()` consumes the binding and returns a `File<M, Closed>`, which is inert and droppable — so `f.close()!` in statement position fully discharges the obligation. Using the old binding afterwards is a compile error.

### Methods on `File<M, Open>`

```
file.read(max_bytes: int) string     // M == Read; "" means EOF; raises FileError
file.write(data: string)             // M == Write; writes ALL of data; raises Degraded
file.sync()                          // M == Write; full durability; raises Degraded
file.sync_data()                     // M == Write; data + size only; raises Degraded
file.seek(to: Seek) int              // either mode; returns new offset; raises FileError
file.close() File<M, Closed>         // consuming transition; raises CloseError
```

`read` requires `max_bytes > 0` (a compile-time contract — see [Contracts](../whats-different/contracts.md)) and returns `""` only at EOF; a failure raises, so an empty string is never a disguised error. `write` loops to completion: success means every byte reached the OS, short writes are never silently dropped.

`seek` takes a `Seek` target instead of whence magic ints, constructed with dotted enum syntax:

```
pub enum Seek {
    Start { offset: int }
    Current { delta: int }
    End { delta: int }
}
```

```
let f = fs.open_read(path)!
let first5 = f.read(5)!
f.seek(fs.Seek.Start { offset: 0 })!
let all = f.read(1024)!
f.close()!
```

`close()` surfaces its result as `CloseError` — but either way the descriptor is gone (POSIX deallocates it even when close fails), so the error is a durability report, not a retry invitation.

### Durability: sync and sync_data

`sync()` means **full durability on every platform**: after it returns, everything previously written through this descriptor is on stable storage — `fsync` on Linux, `fcntl(F_FULLFSYNC)` on Darwin, because Darwin's plain `fsync` does not flush the drive's volatile cache. Strong is the default; if `F_FULLFSYNC` is unsupported (SMB/NFS mounts), the call raises rather than silently degrading. `sync_data()` skips metadata-only flushes (`fdatasync` on Linux; on Darwin there is no honest cheaper form, so it is `F_FULLFSYNC` too).

One honest caveat: the implementation is syscall-faithful, not crash-tested — CI verifies the right syscalls are issued and the error paths are honest; actual power-loss durability depends on the drive's firmware telling the truth.

### Degradation: a failed write or sync poisons the handle

A failed `write` or `sync` destroys the durability warrant for the descriptor: after a failed write the prefix state is unknown, and after a failed sync the kernel has reported the writeback error *once* and marked the pages clean — it cannot re-issue the report, so a retried sync would succeed while your data is gone. (This is the "fsyncgate" failure mode that silently corrupted PostgreSQL data for years.)

`std.fs` encodes the rule in the type system. The failure raises `Degraded`, whose payload carries the handle in its honest post-failure state:

```
pub error Degraded { file: File<Write, Poisoned>, code: int, message: string }
```

Catching `Degraded` consumes the `Open` binding — the value now lives in `e.file` as a `File<Write, Poisoned>`. The **only** method on a poisoned handle is the discharge:

```
file.discard() File<M, Closed>   // the only exit from Poisoned; never raises
```

Calling `sync` or `write` on a poisoned handle is a type error: the method to retry does not exist on the type the failure hands you. And because the payload is itself `must_release`, a wildcard `catch` cannot swallow a `Degraded` — only a typed handler, which takes on the obligation, is legal. Recovery is reopening and rewriting from data the program still owns, or crashing and recovering from a log.

The write-ahead-log shape, from `examples/durable_config`:

```
error ConfigDead { reason: string }

fn append_durably(w: fs.File<fs.Write, fs.Open>, entry: string) fs.File<fs.Write, fs.Open> {
    w.write(entry) catch e: fs.Degraded {
        let p = e.file
        p.discard()
        raise ConfigDead { reason: f"write failed (errno {e.code})" }
    }
    w.sync_data() catch e: fs.Degraded {
        let p = e.file
        p.discard()
        raise ConfigDead { reason: f"sync failed (errno {e.code}); reopen and rewrite" }
    }
    return w
}
```

The obligation travels: in through the parameter, back through the return, and on failure into the error payload, where the handler discards it and reports upstream. The caller holds the handle in a local across batches:

```
let mut wal = fs.open_append(dir + "/changes.log")!
wal = append_durably(wal, "set retries 5\n")!
wal = append_durably(wal, "set timeout_ms 200\n")!
wal.close()!
```

Forgetting `wal.close()` is not a bug to find in review — it is a compile error naming the obligation.

## Metadata

### stat

```
fs.stat(path: string) Metadata   // raises NotFound | FileError
```

```
pub class Metadata {
    size: int        // bytes; invariant: size >= 0
    modified: int    // unix seconds
    is_dir: bool
    is_file: bool
    mode: int        // permission bits
}
```

`Metadata` carries a compile-time invariant `size >= 0`, proven at its construction site inside `std.fs` — callers get to assume it for free.

### Path queries

```
fs.exists(path: string) bool
fs.is_file(path: string) bool
fs.is_dir(path: string) bool
fs.file_size(path: string) int     // = stat(path).size; raises NotFound | FileError
```

## File Operations

```
fs.remove(path: string)                   // delete a file; raises NotFound | FileError
fs.rename(from: string, to: string)       // rename or move; raises NotFound | FileError
fs.copy(from: string, to: string)         // copy; close errors surfaced; raises NotFound | FileError
```

## Directory Operations

```
fs.mkdir(path: string)                    // create one directory; raises NotFound | FileError
fs.create_dir_all(path: string)           // mkdir -p; existing directory is success
fs.rmdir(path: string)                    // remove empty directory; raises NotFound | FileError
fs.remove_dir_all(path: string)           // recursive delete; refuses "/" and ""
fs.list_dir(path: string) [string]        // list entries; raises NotFound | FileError
fs.temp_dir() string                      // system temp directory
```

`remove_dir_all` unlinks symlinks rather than following them.

## Example: Durable Config Update

```
import std.fs

fn main() {
    let dir = fs.temp_dir()
    let config = dir + "/app.conf"

    fs.write_all(config, "retries = 3\n")!
    print(f"before: {fs.read_all(config)!}")

    fs.replace_all(config, "retries = 5\ntimeout_ms = 200\n") catch e: fs.SyncError {
        print(f"replace landed without durability warrant: {e.message}")
        return
    } catch e: fs.FileError {
        print(f"replace failed, old config intact: {e.message}")
        return
    } catch e: fs.NotFound {
        print(f"missing: {e.path}")
        return
    }
    print(f"after:  {fs.read_all(config)!}")

    fs.remove_dir_all(dir)!
}
```

The full version — including the WAL half with `open_append`, per-batch `sync_data`, and honest degradation — is `examples/durable_config/main.pt`; the plain file I/O tour is `examples/file_io/main.pt`.
