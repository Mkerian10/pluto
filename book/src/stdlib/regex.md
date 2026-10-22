# std.regex

Simple pattern matching. Supports literals, `.` (any character), `^`/`$` anchors, `*`/`+`/`?` quantifiers on a single token, the classes `\d` (digit), `\w` (letter, digit or `_`) and `\s` (space, tab, newline), and `\x` for a literal metacharacter (`\.`, `\*`, `\\`). No alternation, groups or bracket classes.

```
import std.regex
```

## Functions

### matches

```
regex.matches(pattern: string, text: string) bool
```

Returns `true` if the pattern matches anywhere in the text.

```
regex.matches("he..o", "hello")  // true
regex.matches("^hi", "hello")   // false
```

### find

```
regex.find(pattern: string, text: string) int
```

Returns the index of the first match, or `-1` if not found.

### find_all

```
regex.find_all(pattern: string, text: string) [int]
```

Returns an array of all match start positions.

### replace

```
regex.replace(pattern: string, replacement: string, text: string) string
```

Replaces the first match with the replacement string.

```
regex.replace("world", "pluto", "hello world")  // "hello pluto"
```

### replace_all

```
regex.replace_all(pattern: string, replacement: string, text: string) string
```

Replaces all matches with the replacement string.

### split

```
regex.split(pattern: string, text: string) [string]
```

Splits the text at each match of the pattern.

```
regex.split(",", "a,b,c")  // ["a", "b", "c"]
```

## Example

```
import std.regex

fn main() {
    let text = "The quick brown fox"
    if regex.matches("qu..k", text) {
        print("Found a match")
    }
    let cleaned = regex.replace_all(" +", " ", "too   many   spaces")
    print(cleaned)
}
```
