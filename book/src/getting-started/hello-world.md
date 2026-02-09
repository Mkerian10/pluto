# Hello, World

Let's write your first Pluto program.

Create a file called `main.pluto`:

```
fn main() {
    print("hello, world")
}
```

Compile and run it:

```bash
plutoc run main.pluto
```

You should see `hello, world` printed to the console.

## What's happening here?

- `fn main()` defines the entry point of your program
- `print()` is a built-in function that writes to stdout
- Pluto has no semicolons -- statements are separated by newlines
