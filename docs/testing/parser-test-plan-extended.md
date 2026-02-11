# Extended Parser Test Plan - 100+ High-Quality Tests

**Inspired by**: Rust compiler test suite, Go parser tests, Pratt parsing best practices
**Goal**: Comprehensive parser coverage with actionable, professional-grade tests
**Status**: Phase 2 Extended - Ready for implementation

---

## Category 1: Operator Precedence & Associativity (25 tests)

### Exhaustive Binary Operator Precedence
1. **Multiplication vs Addition** - `2 + 3 * 4` → `2 + (3 * 4)` = 14
2. **Division vs Subtraction** - `10 - 8 / 2` → `10 - (8 / 2)` = 6
3. **Modulo vs Addition** - `5 + 7 % 3` → `5 + (7 % 3)` = 6
4. **Exponentiation vs Multiplication** (if supported) - `2 * 3 ** 2`
5. **Shift Left vs Addition** - `1 + 2 << 3` → `(1 + 2) << 3` = 24
6. **Shift Right vs Multiplication** - `16 >> 2 * 1` → `16 >> (2 * 1)` = 4
7. **Bitwise AND vs Equality** - `5 & 3 == 1` → `(5 & 3) == 1` = true
8. **Bitwise XOR vs AND** - `5 ^ 3 & 1` → `5 ^ (3 & 1)` = 4
9. **Bitwise OR vs XOR** - `4 | 2 ^ 1` → `4 | (2 ^ 1)` = 7
10. **Comparison vs Bitwise** - `3 < 5 & 1` → should fail type error
11. **Logical AND vs Comparison** - `3 < 5 && 2 > 1` → `(3 < 5) && (2 > 1)` = true
12. **Logical OR vs AND** - `true || false && false` → `true || (false && false)` = true
13. **Equality vs Comparison** - `3 < 5 == true` → `(3 < 5) == true` = true
14. **Inequality vs Equality** - `3 != 4 == true` → `(3 != 4) == true` = true
15. **Range vs Comparison** - `1..10 == 1..10` (if ranges are values)

### Associativity Tests
16. **Left-associative Subtraction Chain** - `10 - 5 - 2 - 1` → `((10 - 5) - 2) - 1` = 2
17. **Left-associative Division Chain** - `100 / 5 / 2` → `(100 / 5) / 2` = 10
18. **Left-associative Modulo Chain** - `15 % 7 % 3` → `(15 % 7) % 3` = 1
19. **Right-associative Assignment** (if supported) - `a = b = c = 5`
20. **Right-associative Exponentiation** (if supported) - `2 ** 3 ** 2` → `2 ** (3 ** 2)` = 512

### Mixed Operator Precedence
21. **Arithmetic + Comparison + Logical** - `2 + 3 > 4 && 5 < 10`
22. **Bitwise + Shift + Comparison** - `8 >> 1 & 3 == 0`
23. **Unary + Binary + Ternary** (if supported) - `-x * 2 + (y > 0 ? 1 : 0)`
24. **Cast + Arithmetic + Comparison** - `x as float + 1.0 > 2.0`
25. **Postfix + Infix + Prefix** - `!arr[0] * -2 + func().field`

---

## Category 2: Expression Complexity & Nesting (20 tests)

### Deep Nesting
26. **20-level Parentheses** - `((((((((((((((((((((42))))))))))))))))))))`
27. **15-level Array Nesting** - `[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]`
28. **10-level Function Call Chain** - `a(b(c(d(e(f(g(h(i(j()))))))))))`
29. **Mixed Nesting** - `arr[obj.method()[0].field]`
30. **Deep Ternary Nesting** (if supported) - `a ? b ? c ? d : e : f : g`

### Expression Combinations
31. **Array Literal of Closures** - `[(x) => x + 1, (x) => x * 2, (x) => x - 3]`
32. **Struct Literal with Closure Fields** - `Handler { f: (x) => x + 1, g: (y) => y * 2 }`
33. **Closure Capturing Closure** - `let outer = 5; (x) => (y) => outer + x + y`
34. **Match Inside Closure** - `(opt) => match opt { Some{v} => v, None => 0 }`
35. **Closure as Function Argument** - `map(arr, (x) => x * 2)`
36. **Chained Method Calls** - `obj.method1().method2().method3().method4()`
37. **Index Chain** - `arr[0][1][2][3]`
38. **Mixed Postfix Operators** - `arr[0]?.field!.method()`
39. **String Interpolation in Expressions** - `"result: {2 + 3 * func(arr[0])}"`
40. **Range in Expression** - `arr[1..arr.len() - 1]`

### Edge Cases
41. **Empty Array Literal** - `[]` with type inference
42. **Empty Map Literal** - `Map<int, int> {}`
43. **Single-element Array with Trailing Comma** - `[1,]`
44. **Multi-line Expression** - Expression spanning 10+ lines
45. **Expression with 50+ Operators** - Long arithmetic expression

---

## Category 3: Statement Boundaries & Newlines (12 tests)

### Newline Significance
46. **Method Call After Newline** - `obj\n.method()`
47. **Binary Operator After Newline** - `x\n+ y`
48. **Chained Calls Across Lines** - Multi-line method chain
49. **Array Access After Newline** - `arr\n[0]`
50. **Generic Type Args After Newline** - `Box\n<int>`

### Statement Termination
51. **Let Without Newline in If** - `if true { let x = 1 let y = 2 }`
52. **Multiple Statements Same Line** - How parser handles this
53. **Return at EOF** - `fn main() { return 42` (no newline at end)
54. **Statement After Closing Brace** - `if true { x } y`
55. **Newline in String Literal** - Multi-line strings
56. **Newline in Comment** - Multi-line comments
57. **Windows Line Endings** - `\r\n` handling

---

## Category 4: Error Recovery & Malformed Input (18 tests)

### Missing Tokens
58. **Missing Opening Paren** - `fn main) {}`
59. **Missing Closing Paren** - `fn main( {}`
60. **Missing Opening Brace** - `fn main()`
61. **Missing Closing Brace** - `fn main() {`
62. **Missing Comma in Params** - `fn foo(x: int y: int)`
63. **Missing Colon in Type** - `let x int = 5`
64. **Missing Equals in Let** - `let x: int 5`
65. **Missing Arrow in Closure** - `(x: int) x + 1`

### Extra/Unexpected Tokens
66. **Extra Comma in Params** - `fn foo(x: int,, y: int)`
67. **Extra Semicolon** - `let x = 5;;`
68. **Unexpected Keyword** - `let fn = 5`
69. **Stray Closing Brace** - `fn main() { } }`
70. **Double Operator** - `x ++ y` (if ++ not supported)

### Incomplete Constructs
71. **Incomplete If** - `if true`
72. **Incomplete While** - `while x < 10`
73. **Incomplete Match** - `match x {`
74. **Incomplete Function** - `fn foo()`
75. **Incomplete Class** - `class Foo`

---

## Category 5: Type Syntax Edge Cases (18 tests)

### Complex Type Expressions
76. **Nested Generic with Map** - `Map<string, Map<int, [string]>>`
77. **Generic with Closure Type** - `Box<fn(int) string>`
78. **Array of Generic** - `[Box<int>]`
79. **Nullable Generic** - `Option<T>?` or `Box<int?>?`
80. **Generic with Multiple Bounds** - `fn foo<T: Trait1 + Trait2 + Trait3>()`
81. **Generic with Nested Bounds** - `fn foo<T: Iterator<Item=U>, U: Display>()`
82. **Function Type with Multiple Params** - `fn(int, float, string, bool) int`
83. **Function Type Returning Function** - `fn(int) fn(int) int`
84. **Deeply Nested Nullable** - Type with 5+ levels of nullable wrapping (if allowed)

### Generic Syntax Edge Cases
85. **Generic with Comparison Conflict** - `x < y > z` vs `Foo<Bar<int>>`
86. **Shift Right in Generic** - `Vec<Vec<int>>` → `>>` token
87. **Generic Call with Comparison** - `foo<int>(x > y)`
88. **Generic Empty Type Args** - `Box<>()`
89. **Generic with Expression in Type** - `Array<N + 1>` (const generics)
90. **Generic with Default Type** - `Box<T = int>`
91. **Generic with Lifetime** (if supported) - `Box<'a, T>`
92. **Generic Tuple Type** - `(int, string, bool)` as type
93. **Generic Result/Option Nesting** - `Result<Option<int>, Error>`

---

## Category 6: Declaration Parsing (15 tests)

### Forward References
94. **Function Calls Before Definition** - Call `bar()` in `foo()`, define `bar()` later
95. **Class Uses Before Definition** - Use `Bar` in `Foo`, define `Bar` later
96. **Mutual Recursion** - Functions `a()` and `b()` call each other
97. **Self-Referential Type** - `class Node { next: Node? }`
98. **Generic Self-Reference** - `class Tree<T> { left: Tree<T>?, right: Tree<T>? }`

### Declaration Ordering
99. **App Before Classes** - `app` declaration before dependencies defined
100. **Trait Before Implementors** - Trait defined, then multiple classes implement
101. **Enum Before Match** - Enum used in match before definition
102. **Error Before Throw** - `error` declaration after `raise` statement
103. **Import Before Use** - Import after items are referenced

### Visibility & Access
104. **Pub on All Declaration Types** - `pub fn`, `pub class`, `pub trait`, `pub enum`
105. **Pub on Struct Fields** - `pub` fields in class (if supported)
106. **Mixed Pub/Private** - Some items pub, others not
107. **Pub Generic** - `pub class Box<T>`
108. **Pub Method in Private Class** - Can method be pub if class isn't?

---

## Category 7: Identifier & Keyword Edge Cases (10 tests)

### Identifier Validation
109. **Unicode Identifiers** - `let π = 3.14`, `let 変数 = 5`
110. **Underscore-only Identifier** - `let _ = 5` or `let __ = 5`
111. **Identifier Starting with Underscore** - `let _temp = 5`
112. **Very Long Identifier** - 1000+ character identifier
113. **Keyword-like Identifiers** - `let class_ = 5`, `let fn_ = 5`

### Reserved Words
114. **Using Reserved Keywords as Identifiers** - Should fail
115. **Contextual Keywords** - Keywords that are valid identifiers in some contexts
116. **Future Reserved Words** - Words reserved for future use
117. **Raw Identifiers** (if supported) - `r#type`, `r#match`
118. **Escaped Identifiers** - Using escape sequences in names

---

## Category 8: Literal Parsing (15 tests)

### Number Literals
119. **Integer Overflow** - `999999999999999999999`
120. **Float Precision** - `0.1 + 0.2 == 0.3` (floating point edge case)
121. **Scientific Notation** - `1e10`, `2.5e-3`
122. **Hex Literals** - `0xFF`, `0x1A2B`
123. **Binary Literals** - `0b1010`, `0b11111111`
124. **Octal Literals** - `0o755`, `0o644`
125. **Underscore Separators** - `1_000_000`, `0xFF_FF_FF`
126. **Leading Zeros** - `007` (should it be octal or error?)

### String Literals
127. **Escape Sequences** - `\n`, `\t`, `\r`, `\\`, `\"`
128. **Unicode Escapes** - `\u{1F600}` (emoji)
129. **Raw Strings** - `r"C:\path\to\file"`
130. **Multi-line Strings** - Triple-quoted strings
131. **Empty String** - `""`
132. **String with Quotes** - `"He said \"hello\""`
133. **String Interpolation Edge Cases** - `"nested {\"inner {x}\"}"`

---

## Category 9: Control Flow Parsing (12 tests)

### If Expressions
134. **If Without Else** - `if cond { x }`
135. **Else If Chain** - 10+ else if branches
136. **If as Expression** - `let x = if cond { 1 } else { 2 }`
137. **If in If Condition** - `if (if x { true } else { false }) { }`
138. **Single-line If** - `if true { print("hi") }`

### Loop Constructs
139. **While with Complex Condition** - `while x < 10 && y > 0 && z != 5`
140. **For with Range Expression** - `for i in 1..arr.len() - 1`
141. **Nested Loops (5 levels)** - Deep loop nesting
142. **Loop with Break in Middle** - Multiple break points
143. **Loop with Continue** - Multiple continue points
144. **Infinite Loop** - `while true { }` or `loop { }` (if supported)

### Match Expressions
145. **Match All Enum Variants** - Exhaustive match
146. **Match with Guards** - `match x { Some{v} if v > 0 => ... }`
147. **Match Nested Patterns** - `match opt { Some{Some{x}} => x, ... }`

---

## Category 10: Pattern Matching (10 tests)

### Match Patterns
148. **Wildcard Pattern** - `match x { _ => 0 }`
149. **Literal Patterns** - `match x { 1 => ..., 2 => ..., _ => ... }`
150. **Multiple Patterns** - `match x { 1 | 2 | 3 => ... }`
151. **Range Patterns** - `match x { 1..10 => ..., _ => ... }`
152. **Struct Destructuring** - `match point { Point{x, y} => ... }`
153. **Nested Destructuring** - `match opt { Some{Point{x, y}} => ... }`
154. **Array Destructuring** - `match arr { [first, second, ..rest] => ... }`
155. **Enum Destructuring** - `match result { Ok{v} => v, Err{e} => 0 }`
156. **Guard with Destructuring** - `match opt { Some{x} if x > 0 => ... }`
157. **Irrefutable Pattern** - Patterns that always match

---

## Category 11: Comment & Documentation (8 tests)

### Comment Placement
158. **Comment Before Function** - Doc comment above function
159. **Comment Inside Function** - Inline comments
160. **Comment at End of Line** - Trailing comment
161. **Comment Between Tokens** - `fn /* comment */ main()`
162. **Nested Block Comments** - `/* outer /* inner */ outer */` (if supported)
163. **Comment in Expression** - `x + /* comment */ y`
164. **Empty Comment** - `//` or `/* */`
165. **Very Long Comment** - 10,000+ character comment

---

## Category 12: Module System Edge Cases (10 tests)

### Import Variations
166. **Import with Path** - `import std.collections.map`
167. **Import Multiple** - `import math, strings, collections`
168. **Circular Imports** - Module A imports B, B imports A
169. **Import Non-existent Module** - Error handling
170. **Import After Code** - Import not at top of file (should fail)

### Visibility
171. **Private Item in Pub Module** - Mixed visibility
172. **Pub Item in Private Module** - Can't access from outside
173. **Re-export** - `pub use internal.Foo`
174. **Import Aliasing** - `import math as m`
175. **Wildcard Import** - `import std.collections.*` (if supported)

---

## Category 13: App & DI Edge Cases (10 tests)

### App Declaration
176. **App with No Dependencies** - `app MyApp { fn main(self) {} }`
177. **App with 10+ Dependencies** - Complex DI graph
178. **App with Circular Dependencies** - Should error
179. **App Defined After Dependencies** - Forward reference
180. **Multiple App Declarations** - Should error

### Dependency Injection
181. **Scoped Class with DI** - `scoped class Handler[db: Database]`
182. **Generic Class with DI** - `class Repo<T>[db: Database]`
183. **Trait as Dependency** - DI with trait type
184. **Nullable Dependency** - `class Foo[db: Database?]`
185. **Optional DI** - Some deps injected, some not

---

## Category 14: Concurrency & Async (10 tests)

### Spawn & Tasks
186. **Spawn Simple Function** - `spawn foo()`
187. **Spawn with Arguments** - `spawn compute(x, y, z)`
188. **Spawn Returning Complex Type** - `spawn get_data(): Map<string, [int]>`
189. **Multiple Spawns** - 10+ concurrent tasks
190. **Spawn in Loop** - Create tasks in loop
191. **Task.get() Chain** - `task1.get() + task2.get()`
192. **Spawn with Error Handling** - `spawn fallible()!`
193. **Nested Spawn** - Spawn task that spawns task (if allowed)

### Channels (if implemented)
194. **Channel Send/Receive** - Basic channel operations
195. **Channel in Loop** - Producer/consumer pattern

---

## Category 15: Error Handling Syntax (10 tests)

### Error Propagation
196. **Propagate in Expression** - `foo()! + bar()!`
197. **Propagate in If Condition** - `if check()! { }`
198. **Propagate in Match** - `match get_value()! { }`
199. **Propagate in Return** - `return compute()!`
200. **Nested Propagation** - `outer(inner()!)!`

### Catch Syntax
201. **Catch All Errors** - `foo() catch { 0 }`
202. **Catch Specific Error** - `foo() catch MathError { 0 }`
203. **Catch in Expression** - `x + (foo() catch { 0 })`
204. **Catch with Destructuring** - `foo() catch MathError{msg} { print(msg); 0 }`
205. **Multiple Catch** - `foo() catch E1 { } catch E2 { }`

---

## Bonus: Stress Tests (10 tests)

206. **10,000 Line File** - Very large file
207. **1000 Function Definitions** - Mass declarations
208. **Deeply Nested Scopes (50 levels)** - Stack stress
209. **Expression with 1000 Operators** - Parsing complexity
210. **Array with 10,000 Elements** - Literal parsing stress
211. **String with 100,000 Characters** - Large string literal
212. **1000 Generic Type Parameters** - `fn foo<T1, T2, ..., T1000>()`
213. **Match with 1000 Arms** - Exhaustiveness checking
214. **File with Only Whitespace** - 10,000 spaces/newlines
215. **Maximum Identifier Length** - System limits

---

## Summary

**Total Tests**: 215 high-quality, actionable parser tests

**Distribution**:
- Operator precedence & associativity: 25 tests
- Expression complexity: 20 tests
- Statement boundaries: 12 tests
- Error recovery: 18 tests
- Type syntax: 18 tests
- Declarations: 15 tests
- Identifiers & keywords: 10 tests
- Literals: 15 tests
- Control flow: 12 tests
- Pattern matching: 10 tests
- Comments: 8 tests
- Module system: 10 tests
- App & DI: 10 tests
- Concurrency: 10 tests
- Error handling: 10 tests
- Stress tests: 10 tests

**Implementation Priority**:
1. Categories 1-2: Operator precedence and expression complexity (foundational)
2. Categories 4-5: Error recovery and type syntax (high value)
3. Categories 6-7: Declarations and identifiers (correctness)
4. Categories 8-10: Literals, control flow, patterns (coverage)
5. Categories 11-15: Comments, modules, advanced features (completeness)
6. Bonus: Stress tests (robustness)

**Testing Strategy**:
- Implement in batches of 25-30 tests
- Use property-based testing for stress tests
- Focus on tests that reveal bugs, not just increase coverage
- Document expected behavior from Rust/Go when ambiguous
- Create fuzzing harness based on these patterns
