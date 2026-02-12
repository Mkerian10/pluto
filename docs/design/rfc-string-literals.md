# RFC: String Literals and Interpolation

**Status:** Accepted  
**Author:** Design discussion  
**Date:** 2026-02-11

## Summary

Split strings into two types:
1. **Regular strings** `"foo"` — no interpolation, just literal text
2. **F-strings** `f"foo {x}"` — explicit interpolation with `{expr}` syntax
3. **Triple strings** `"""..."""` — deferred for future encoding/rich object design

## Motivation

Current behavior (all strings interpolate) causes problems:
- Writing JSON requires escaping: `"{{\"key\": \"value\"}}"`
- Not obvious when interpolation happens
- Escaping with `{{` and `}}` is undocumented

New behavior makes interpolation **opt-in** via `f` prefix.

## Design

### Regular Strings `"..."`

No interpolation. C-style escapes only:

```pluto
let json = "{"key": "value"}"        // ✅ Literal braces, no interpolation
let msg = "Hello {name}"             // ✅ Literal text, no interpolation
let quote = "She said \"hello\""     // ✅ Escape quote with \"
let multiline = "line1\nline2\ttab"  // ✅ \n, \t, \r work
```

**Escape sequences:**
- `\"` → `"` (quote)
- `\\` → `\` (backslash)  
- `\n` → newline
- `\t` → tab
- `\r` → carriage return

### F-Strings `f"..."`

Explicit interpolation with `{expr}` syntax:

```pluto
let name = "Alice"
let age = 30
let msg = f"Hello {name}, you are {age} years old"
// → "Hello Alice, you are 30 years old"

let json = f"{{\"name\": \"{name}\"}}"  // Escape braces with {{ and }}
// → {"name": "Alice"}
```

**Escape sequences:**
- All C-style escapes from regular strings
- `{{` → `{` (literal left brace)
- `}}` → `}` (literal right brace)

**Interpolation rules:**
- `{expr}` — any expression (variable, method call, arithmetic, etc.)
- Nested braces count depth: `{obj.method()}`
- Empty `{}` is an error

### Triple Strings `"""..."""`

**Deferred** — will design as part of encoding/decoding system.

Vision: Compose structured objects with automatic encoding:
```pluto
let cat = Cat{name: "Fluffy", age: 3}
let doc = """
{
  "cat": {cat},
  "timestamp": {now()}
}
"""
// doc is a structured object, not a string
```

Details TBD when we design the encoding system.

## Migration Path

### Phase 1: Add F-Strings (Non-Breaking)
- Implement `f"..."` syntax
- Regular `"..."` still interpolates (backward compatible)
- Both work, no code breaks

### Phase 2: Break Regular Strings (Breaking Change)
- Remove interpolation from `"..."`
- Only `f"..."` interpolates
- Update all existing Pluto code

**Timeline:** Phase 1 now, Phase 2 when ready (accept the breakage)

## Implementation

### Lexer Changes
Add new token for f-strings:

```rust
// In src/lexer/token.rs
#[token("f\"", parse_fstring)]
FStringStart,

// Parse f"..." as FStringLit
FStringLit(String),
```

### Parser Changes
- Parse `FStringLit` same as current `StringLit` (interpolation logic)
- Eventually: parse `StringLit` as literal (no interpolation)

### Error Messages
F-strings:
```
error: Unterminated f-string interpolation
  --> file.pluto:5:15
   |
 5 | let s = f"Hello {name"
   |                 ^ expected closing '}'
   |
help: f-strings use {expr} syntax for interpolation
help: use {{ for a literal '{' character
```

Regular strings (after Phase 2):
```
error: Unexpected '{' in string literal
  --> file.pluto:5:15
   |
 5 | let s = "Hello {name}"
   |                ^ interpolation not supported in regular strings
   |
help: use f"..." for string interpolation: f"Hello {name}"
```

## Examples

### Before (Current)
```pluto
let msg = "Hello {name}"              // Interpolates
let json = "{{\"key\": \"{value}\"}}" // Must escape braces
```

### After Phase 1 (F-Strings Added)
```pluto
let msg = "Hello {name}"              // Still interpolates (backward compat)
let msg2 = f"Hello {name}"            // Also interpolates (new syntax)
let json = "{{\"key\": \"{value}\"}}" // Still need escaping in either
let json2 = f"{{\"key\": \"{value}\"}}" // Same in f-strings
```

### After Phase 2 (Regular Strings Broken)
```pluto
let msg = "Hello {name}"              // ❌ Error: no interpolation
let msg = f"Hello {name}"             // ✅ Use f-string
let json = "{"key": "value"}"         // ✅ No escaping needed!
let json = f"{{\"key\": \"{value}\"}}" // ✅ F-string with interpolation
```

## Testing

Add tests for:
- [x] F-string interpolation: `f"Hello {name}"`
- [x] F-string escaping: `f"Use {{ for brace"`
- [x] Nested expressions: `f"{obj.method(x + 1)}"`
- [ ] Mixed content: `f"Price: ${{{price}}}"`
- [ ] Edge cases: empty `f"{}"`, whitespace `f"{ x }"`
- [ ] Error messages for unterminated/malformed f-strings

Phase 2 tests:
- [ ] Regular strings reject interpolation
- [ ] Regular strings allow literal braces
- [ ] Clear error messages pointing to f-strings

## Open Questions

None — proceed with implementation.

## Decision

**Accepted:** Implement Phase 1 (f-strings) immediately.

**Next:** Create implementation branch and start coding.

---

## Changelog

- 2026-02-11: Initial RFC approved after design discussion
