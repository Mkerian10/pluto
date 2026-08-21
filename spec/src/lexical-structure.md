# Lexical Structure

This chapter defines the lexical grammar of Pluto — the rules for how source text is divided into tokens before parsing.

## Source Encoding

Pluto source files must be valid UTF-8. There is no byte-order mark (BOM) requirement.

## Comments

Pluto supports two forms of comments:

```
// This is a line comment. It extends to the end of the line.

/* This is a block comment.
   It can span multiple lines. */
```

Line comments begin with `//` and extend to the end of the line.

Block comments begin with `/*` and end with the first subsequent `*/`. Block comments do not nest — `/* outer /* inner */ still open */` is a syntax error because the comment ends at the first `*/`.

Comments are stripped during lexing and have no effect on program behavior.

## Whitespace

Horizontal whitespace (spaces and tabs) is not significant and is ignored during lexing.

Newlines are significant. Pluto uses newline-based statement termination — there are no semicolons. A newline (`\n` or `\r\n`) terminates a statement unless the parser determines that the current expression continues on the next line (e.g., after an operator or open delimiter).

Consecutive newlines are treated as a single newline token.

## Identifiers

Identifiers name variables, functions, types, fields, and other program entities.

```
identifier = XID_Start XID_Continue*
```

An identifier begins with a character in the Unicode `XID_Start` category (letters and underscore) followed by zero or more characters in the `XID_Continue` category (letters, digits, underscores, and combining marks).

Identifiers are compared byte-for-byte with no Unicode normalization. Two identifiers that look visually identical but differ in their UTF-8 encoding are distinct identifiers.

Emoji characters are not valid in identifiers.

Identifiers are case-sensitive: `foo`, `Foo`, and `FOO` are three distinct identifiers.

## Keywords

The following words are reserved and cannot be used as identifiers:

```
app        ambient    as         assert     break      catch
class      continue   default    else       enum       error
extern     false      fn         for        if         impl
import     in         inject     invariant  let        match
mut        none       override   pub        raise      requires
return     scope      scoped     select     self       spawn
stage      stream     system     test       tests      trait
transient  true       uses       while      yield
```

## Operators and Punctuation

### Arithmetic Operators

| Token | Name |
|-------|------|
| `+`   | Addition |
| `-`   | Subtraction / Negation |
| `*`   | Multiplication |
| `/`   | Division |
| `%`   | Remainder |

### Compound Assignment Operators

| Token | Name |
|-------|------|
| `+=`  | Add-assign |
| `-=`  | Subtract-assign |
| `*=`  | Multiply-assign |
| `/=`  | Divide-assign |
| `%=`  | Remainder-assign |

### Increment / Decrement Operators

| Token | Name |
|-------|------|
| `++`  | Increment |
| `--`  | Decrement |

### Comparison Operators

| Token | Name |
|-------|------|
| `==`  | Equal |
| `!=`  | Not equal |
| `<`   | Less than |
| `>`   | Greater than |
| `<=`  | Less than or equal |
| `>=`  | Greater than or equal |

### Logical Operators

| Token | Name |
|-------|------|
| `&&`  | Logical AND |
| `\|\|`  | Logical OR |
| `!`   | Logical NOT (prefix) / Error propagation (postfix) |

### Bitwise Operators

| Token | Name |
|-------|------|
| `&`   | Bitwise AND |
| `\|`   | Bitwise OR |
| `^`   | Bitwise XOR |
| `~`   | Bitwise NOT |
| `<<`  | Left shift |

Right shift (`>>`) is not a single token. It is parsed as two consecutive `>` tokens to avoid ambiguity with nested generic type arguments (e.g., `Map<string, Box<int>>`).

### Punctuation

| Token | Name |
|-------|------|
| `(`   | Left parenthesis |
| `)`   | Right parenthesis |
| `{`   | Left brace |
| `}`   | Right brace |
| `[`   | Left bracket |
| `]`   | Right bracket |
| `,`   | Comma |
| `:`   | Colon |
| `::`  | Double colon |
| `.`   | Dot |
| `..`  | Exclusive range |
| `..=` | Inclusive range |
| `->`  | Arrow (used in `fn` types) |
| `=>`  | Fat arrow (closures, match arms) |
| `=`   | Assignment |
| `?`   | Null propagation (postfix) / Nullable type (suffix) |

## Literals

### Integer Literals

Integer literals represent values of type `int` (64-bit signed).

```
decimal_lit     = [0-9] [0-9_]*
hex_lit         = "0" ("x" | "X") hex_digit [hex_digit | "_"]*
binary_lit      = "0" ("b" | "B") bin_digit [bin_digit | "_"]*

hex_digit       = [0-9 a-f A-F]
bin_digit       = [0-1]
```

Underscores may appear between any two digits for readability. Leading or trailing underscores are not permitted.

Examples:
```
42
1_000_000
0xFF
0xFF_AA
0b1010
0b1111_0000
```

Integer literals must fit within the range of a signed 64-bit integer: −9,223,372,036,854,775,808 to 9,223,372,036,854,775,807.

### Float Literals

Float literals represent values of type `float` (64-bit IEEE 754 double precision).

```
float_lit       = decimal_digits "." decimal_digits exponent?
                | decimal_digits exponent

decimal_digits  = [0-9] [0-9_]*
exponent        = ("e" | "E") ("+" | "-")? decimal_digits
```

A float literal must have either a decimal point with digits on both sides, or an exponent. A bare integer with an exponent (e.g., `1e6`) is a valid float literal.

Examples:
```
3.14
0.5
1_000.5
1e6
2.5e-3
1_000e6
```

### Boolean Literals

```
true
false
```

The literals `true` and `false` represent the two values of type `bool`.

### String Literals

Pluto has three forms of string literal:

#### Regular Strings

```
string_lit = '"' string_char* '"'
```

Regular strings are delimited by double quotes and support the following escape sequences:

| Escape | Value |
|--------|-------|
| `\n`   | Newline (U+000A) |
| `\r`   | Carriage return (U+000D) |
| `\t`   | Tab (U+0009) |
| `\\`   | Backslash |
| `\"`   | Double quote |
| `\0`   | Null byte (U+0000) |
| `\xNN` | Byte value (exactly 2 hex digits) |
| `\u{N..N}` | Unicode codepoint (1-6 hex digits) |

Unicode escape values must be valid Unicode scalar values. Surrogate codepoints (U+D800 through U+DFFF) and values above U+10FFFF are rejected.

#### Interpolated Strings

```
fstring_lit = 'f"' fstring_part* '"'
fstring_part = string_char | '{' expression '}'
```

Interpolated strings are prefixed with `f` and allow arbitrary Pluto expressions inside `{` `}` delimiters. The expressions are evaluated at runtime and their results are converted to strings.

Examples:
```
f"Hello, {name}!"
f"The sum is {a + b}"
f"User {user.name} has {items.len()} items"
```

Escape sequences work identically to regular strings within the non-interpolated portions.

#### Data Literals

```
data_lit = '"""' content '"""'
```

Data literals are delimited by triple double quotes. They represent compile-time structured data that is validated against a target type. The content format is inferred based on the encoding system (see [Encodings]()).

Data literals do not support escape sequences or interpolation. The content between the delimiters is taken verbatim.

```
let user: User = """
{
    "name": "Alice",
    "age": 30
}
"""
```

### The `none` Literal

The literal `none` represents the absence of a value. It is analogous to `null` in other languages.

`none` has type `T?` where `T` is inferred from context. It is not a value of any specific type — it is a polymorphic literal that takes on the nullable form of whatever type is expected.

```
let x: int? = none       // none has type int?
let y: string? = none    // none has type string?

fn find(id: int) User? {
    return none           // none has type User?
}
```

Using `none` without sufficient type context is a compile error.
