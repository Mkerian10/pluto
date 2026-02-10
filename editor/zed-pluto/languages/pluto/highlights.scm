; Keywords
[
  "fn"
  "let"
  "return"
  "if"
  "else"
  "while"
  "for"
  "in"
  "match"
  "class"
  "trait"
  "enum"
  "error"
  "app"
  "import"
  "extern"
  "test"
  "spawn"
  "raise"
  "catch"
  "uses"
  "ambient"
  "impl"
  "as"
] @keyword

; break/continue are single-token rules, use named nodes
(break_statement) @keyword
(continue_statement) @keyword

; Keyword modifiers
[
  "pub"
  "mut"
] @keyword.modifier

; Booleans
(boolean) @constant.builtin

; Self
(self_) @variable.builtin

; Function definitions
(function_definition
  name: (identifier) @function)

; Function calls
(call_expression
  function: (identifier) @function.call)

; Method calls
(method_call_expression
  method: (identifier) @function.method.call)

; Type names in definitions
(class_definition
  name: (identifier) @type)
(trait_definition
  name: (identifier) @type)
(enum_definition
  name: (identifier) @type)
(error_definition
  name: (identifier) @type)
(app_definition
  name: (identifier) @type)

; Built-in types
((identifier) @type.builtin
  (#match? @type.builtin "^(int|float|bool|string|void)$"))

; Type references
(named_type (identifier) @type)
(generic_type (identifier) @type)
(array_type "Array" @type.builtin)
(qualified_type (identifier) @type)

; Map and Set constructors
(map_literal "Map" @type.builtin)
(set_literal "Set" @type.builtin)

; Parameters
(parameter
  name: (identifier) @variable.parameter)

; Fields
(field_definition
  name: (identifier) @property)
(field_expression
  field: (identifier) @property)
(struct_field_value
  name: (identifier) @property)
(pattern_field
  name: (identifier) @property)

; Bracket dependencies
(bracket_dep
  name: (identifier) @variable.parameter)

; Enum variants
(enum_variant
  name: (identifier) @constant)
(enum_expression
  variant: (identifier) @constant)
(match_arm
  variant: (identifier) @constant)

; Impl clause trait names
(impl_clause (identifier) @type)
(uses_clause (identifier) @type)
(ambient_declaration (identifier) @type)

; Import name
(import_declaration (identifier) @module)

; Extern rust alias
(extern_rust_declaration
  alias: (identifier) @module)

; Test name
(test_definition
  name: (string) @string)

; Literals
(integer) @number
(float) @number.float
(string) @string

; Comments
(comment) @comment

; Operators
(binary_expression operator: _ @operator)
(unary_expression operator: _ @operator)
(assignment_statement operator: _ @operator)
(range_expression operator: _ @operator)
(propagate_expression "!" @operator)

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter
"=>" @punctuation.special
