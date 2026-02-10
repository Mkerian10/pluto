/// <reference types="tree-sitter-cli/dsl" />

module.exports = grammar({
  name: "pluto",

  extras: ($) => [/[ \t\r]/, $.comment],

  conflicts: ($) => [
    [$.named_type, $.generic_type],
    [$.named_type, $.qualified_type],
  ],

  word: ($) => $.identifier,

  rules: {
    source_file: ($) =>
      seq(
        repeat(seq(repeat($._newline), $._top_level_item)),
        repeat($._newline),
      ),

    _newline: (_) => token(/\n/),

    comment: (_) => token(seq("//", /[^\n]*/)),

    // ─── Top-level items ───────────────────────────────────────────

    _top_level_item: ($) =>
      choice(
        $.function_definition,
        $.class_definition,
        $.trait_definition,
        $.enum_definition,
        $.error_definition,
        $.app_definition,
        $.import_declaration,
        $.extern_fn_declaration,
        $.extern_rust_declaration,
        $.test_definition,
      ),

    import_declaration: ($) =>
      seq("import", $.identifier),

    extern_fn_declaration: ($) =>
      prec.right(
        seq(
          "extern",
          "fn",
          field("name", $.identifier),
          $.parameter_list,
          optional(field("return_type", $._type)),
        ),
      ),

    extern_rust_declaration: ($) =>
      seq(
        "extern",
        "rust",
        field("path", $.string),
        "as",
        field("alias", $.identifier),
      ),

    // ─── Functions ─────────────────────────────────────────────────

    function_definition: ($) =>
      seq(
        optional("pub"),
        "fn",
        field("name", $.identifier),
        optional($.type_parameters),
        $.parameter_list,
        optional(field("return_type", $._type)),
        choice($.block, $._newline),
      ),

    parameter_list: ($) =>
      seq(
        "(",
        optional($._newlines),
        optional(
          seq(
            $.parameter,
            repeat(seq(",", optional($._newlines), $.parameter)),
            optional(","),
            optional($._newlines),
          ),
        ),
        ")",
      ),

    parameter: ($) =>
      seq(
        optional("mut"),
        field("name", choice($.identifier, $.self_)),
        ":",
        field("type", $._type),
      ),

    type_parameters: ($) =>
      seq("<", $.identifier, repeat(seq(",", $.identifier)), ">"),

    // ─── Classes ───────────────────────────────────────────────────

    class_definition: ($) =>
      seq(
        optional("pub"),
        "class",
        field("name", $.identifier),
        optional($.type_parameters),
        optional($.impl_clause),
        optional($.uses_clause),
        optional($.bracket_deps),
        "{",
        repeat($._newline),
        repeat($.class_member),
        "}",
      ),

    impl_clause: ($) =>
      seq("impl", $.identifier, repeat(seq(",", $.identifier))),

    uses_clause: ($) =>
      seq("uses", $.identifier, repeat(seq(",", $.identifier))),

    bracket_deps: ($) =>
      seq(
        "[",
        optional($._newlines),
        $.bracket_dep,
        repeat(seq(",", optional($._newlines), $.bracket_dep)),
        optional(","),
        optional($._newlines),
        "]",
      ),

    bracket_dep: ($) =>
      seq(field("name", $.identifier), ":", field("type", $._type)),

    class_member: ($) =>
      seq(
        choice($.field_definition, $.function_definition),
        repeat($._newline),
      ),

    field_definition: ($) =>
      seq(field("name", $.identifier), ":", field("type", $._type)),

    // ─── Traits ────────────────────────────────────────────────────

    trait_definition: ($) =>
      seq(
        optional("pub"),
        "trait",
        field("name", $.identifier),
        "{",
        repeat($._newline),
        repeat($.trait_member),
        "}",
      ),

    trait_member: ($) =>
      seq($.function_definition, repeat($._newline)),

    // ─── Enums ─────────────────────────────────────────────────────

    enum_definition: ($) =>
      seq(
        optional("pub"),
        "enum",
        field("name", $.identifier),
        optional($.type_parameters),
        "{",
        repeat($._newline),
        repeat(seq($.enum_variant, repeat($._newline))),
        "}",
      ),

    enum_variant: ($) =>
      seq(
        field("name", $.identifier),
        optional(
          seq(
            "{",
            optional($._newlines),
            optional(
              seq(
                $.field_definition,
                repeat(seq(",", optional($._newlines), $.field_definition)),
                optional(","),
                optional($._newlines),
              ),
            ),
            "}",
          ),
        ),
      ),

    // ─── Errors ────────────────────────────────────────────────────

    error_definition: ($) =>
      seq(
        optional("pub"),
        "error",
        field("name", $.identifier),
        "{",
        repeat($._newline),
        repeat(seq($.field_definition, repeat($._newline))),
        "}",
      ),

    // ─── App ───────────────────────────────────────────────────────

    app_definition: ($) =>
      seq(
        "app",
        field("name", $.identifier),
        optional($.bracket_deps),
        "{",
        repeat($._newline),
        repeat($.app_member),
        "}",
      ),

    app_member: ($) =>
      seq(
        choice($.ambient_declaration, $.function_definition),
        repeat($._newline),
      ),

    ambient_declaration: ($) => seq("ambient", $.identifier),

    // ─── Test ──────────────────────────────────────────────────────

    test_definition: ($) =>
      seq("test", field("name", $.string), $.block),

    // ─── Blocks & Statements ───────────────────────────────────────

    block: ($) =>
      seq("{", repeat($._newline), repeat($._statement_with_newline), "}"),

    _statement_with_newline: ($) =>
      seq($._statement, repeat1($._newline)),

    _statement: ($) =>
      choice(
        $.let_statement,
        $.return_statement,
        $.if_statement,
        $.while_statement,
        $.for_statement,
        $.match_statement,
        $.raise_statement,
        $.break_statement,
        $.continue_statement,
        $.assignment_statement,
        $.expression_statement,
      ),

    let_statement: ($) =>
      seq(
        "let",
        optional("mut"),
        field("name", $.identifier),
        optional(seq(":", field("type", $._type))),
        "=",
        field("value", $._expression),
      ),

    return_statement: ($) =>
      seq("return", optional($._expression)),

    if_statement: ($) =>
      seq(
        "if",
        field("condition", $._expression),
        field("consequence", $.block),
        optional(
          seq(
            "else",
            field(
              "alternative",
              choice($.if_statement, $.block),
            ),
          ),
        ),
      ),

    while_statement: ($) =>
      seq("while", field("condition", $._expression), $.block),

    for_statement: ($) =>
      seq(
        "for",
        field("variable", $.identifier),
        "in",
        field("iterable", $._expression),
        $.block,
      ),

    match_statement: ($) =>
      seq(
        "match",
        field("subject", $._expression),
        "{",
        repeat($._newline),
        repeat(seq($.match_arm, repeat($._newline))),
        "}",
      ),

    match_arm: ($) =>
      seq(
        field("enum_name", $.identifier),
        ".",
        field("variant", $.identifier),
        optional($.pattern_fields),
        $.block,
      ),

    pattern_fields: ($) =>
      prec(1,
        seq(
          "{",
          $.pattern_field,
          repeat(seq(",", $.pattern_field)),
          optional(","),
          "}",
        ),
      ),

    pattern_field: ($) =>
      seq(
        field("name", $.identifier),
        ":",
        field("binding", $.identifier),
      ),

    raise_statement: ($) =>
      seq("raise", $._expression),

    break_statement: (_) => "break",

    continue_statement: (_) => "continue",

    assignment_statement: ($) =>
      seq(
        field("target", $._assignable),
        field("operator", choice("=", "+=", "-=", "*=", "/=", "%=")),
        field("value", $._expression),
      ),

    _assignable: ($) =>
      choice(
        $.identifier,
        $.field_expression,
        $.index_expression,
      ),

    expression_statement: ($) => $._expression,

    // ─── Expressions ───────────────────────────────────────────────

    _expression: ($) =>
      choice(
        $.binary_expression,
        $.unary_expression,
        $.call_expression,
        $.method_call_expression,
        $.field_expression,
        $.index_expression,
        $.propagate_expression,
        $.catch_expression,
        $.cast_expression,
        $.spawn_expression,
        $.range_expression,
        $.closure_expression,
        $.struct_literal,
        $.enum_expression,
        $.primary_expression,
      ),

    binary_expression: ($) => {
      const table = [
        [prec.left, 1, "||"],
        [prec.left, 2, "&&"],
        [prec.left, 3, "|"],
        [prec.left, 4, "^"],
        [prec.left, 5, "&"],
        [prec.left, 6, choice("==", "!=")],
        [prec.left, 7, choice("<", ">", "<=", ">=")],
        [prec.left, 8, choice("<<", ">>")],
        [prec.left, 9, choice("+", "-")],
        [prec.left, 10, choice("*", "/", "%")],
      ];
      return choice(
        ...table.map(([fn, p, ops]) =>
          fn(
            p,
            seq(
              field("left", $._expression),
              field("operator", ops),
              optional($._newlines),
              field("right", $._expression),
            ),
          ),
        ),
      );
    },

    unary_expression: ($) =>
      prec(11, seq(field("operator", choice("-", "!", "~")), field("operand", $._expression))),

    call_expression: ($) =>
      prec(
        13,
        seq(
          field("function", $.identifier),
          $.argument_list,
        ),
      ),

    method_call_expression: ($) =>
      prec.left(
        13,
        seq(
          field("object", $._expression),
          ".",
          field("method", $.identifier),
          $.argument_list,
        ),
      ),

    field_expression: ($) =>
      prec.left(13, seq(field("object", $._expression), ".", field("field", $.identifier))),

    index_expression: ($) =>
      prec(
        13,
        seq(
          field("object", $._expression),
          "[",
          optional($._newlines),
          field("index", $._expression),
          optional($._newlines),
          "]",
        ),
      ),

    propagate_expression: ($) =>
      prec(12, seq(field("expression", $._expression), "!")),

    catch_expression: ($) =>
      prec.right(
        0,
        seq(
          field("expression", $._expression),
          "catch",
          field("handler", $._expression),
        ),
      ),

    cast_expression: ($) =>
      prec.left(
        12,
        seq(field("expression", $._expression), "as", field("type", $._type)),
      ),

    spawn_expression: ($) =>
      prec.right(0, seq("spawn", field("call", $.call_expression))),

    range_expression: ($) =>
      prec.left(
        0,
        seq(
          field("start", $._expression),
          field("operator", choice("..", "..=")),
          field("end", $._expression),
        ),
      ),

    closure_expression: ($) =>
      prec.right(
        0,
        seq(
          $.closure_parameters,
          "=>",
          optional($._newlines),
          field("body", choice($.block, $._expression)),
        ),
      ),

    closure_parameters: ($) =>
      seq(
        "(",
        optional($._newlines),
        optional(
          seq(
            $.parameter,
            repeat(seq(",", optional($._newlines), $.parameter)),
            optional(","),
            optional($._newlines),
          ),
        ),
        ")",
      ),

    struct_literal: ($) =>
      prec(
        14,
        seq(
          field("name", $.identifier),
          "{",
          optional($._newlines),
          optional(
            seq(
              $.struct_field_value,
              repeat(seq(",", optional($._newlines), $.struct_field_value)),
              optional(","),
              optional($._newlines),
            ),
          ),
          "}",
        ),
      ),

    struct_field_value: ($) =>
      seq(field("name", $.identifier), ":", field("value", $._expression)),

    enum_expression: ($) =>
      prec.left(
        14,
        seq(
          field("enum_name", $.identifier),
          ".",
          field("variant", $.identifier),
          optional(
            seq(
              "{",
              optional($._newlines),
              optional(
                seq(
                  $.struct_field_value,
                  repeat(
                    seq(",", optional($._newlines), $.struct_field_value),
                  ),
                  optional(","),
                  optional($._newlines),
                ),
              ),
              "}",
            ),
          ),
        ),
      ),

    argument_list: ($) =>
      seq(
        "(",
        optional($._newlines),
        optional(
          seq(
            $._expression,
            repeat(seq(",", optional($._newlines), $._expression)),
            optional(","),
            optional($._newlines),
          ),
        ),
        ")",
      ),

    primary_expression: ($) =>
      choice(
        $.identifier,
        $.integer,
        $.float,
        $.string,
        $.boolean,
        $.self_,
        $.array_literal,
        $.map_literal,
        $.set_literal,
        $.parenthesized_expression,
      ),

    parenthesized_expression: ($) =>
      seq("(", optional($._newlines), $._expression, optional($._newlines), ")"),

    array_literal: ($) =>
      seq(
        "[",
        optional($._newlines),
        optional(
          seq(
            $._expression,
            repeat(seq(",", optional($._newlines), $._expression)),
            optional(","),
            optional($._newlines),
          ),
        ),
        "]",
      ),

    map_literal: ($) =>
      seq(
        "Map",
        "<",
        field("key_type", $._type),
        ",",
        field("value_type", $._type),
        ">",
        "{",
        optional($._newlines),
        optional(
          seq(
            $.map_entry,
            repeat(seq(",", optional($._newlines), $.map_entry)),
            optional(","),
            optional($._newlines),
          ),
        ),
        "}",
      ),

    map_entry: ($) =>
      seq(field("key", $._expression), ":", field("value", $._expression)),

    set_literal: ($) =>
      seq(
        "Set",
        "<",
        field("element_type", $._type),
        ">",
        "{",
        optional($._newlines),
        optional(
          seq(
            $._expression,
            repeat(seq(",", optional($._newlines), $._expression)),
            optional(","),
            optional($._newlines),
          ),
        ),
        "}",
      ),

    // ─── Types ─────────────────────────────────────────────────────

    _type: ($) =>
      choice(
        $.named_type,
        $.array_type,
        $.function_type,
        $.generic_type,
        $.qualified_type,
      ),

    named_type: ($) => $.identifier,

    array_type: ($) => seq("Array", "<", $._type, ">"),

    function_type: ($) =>
      prec.right(
        seq(
          "fn",
          "(",
          optional(seq($._type, repeat(seq(",", $._type)))),
          ")",
          optional($._type),
        ),
      ),

    generic_type: ($) =>
      seq(
        $.identifier,
        "<",
        $._type,
        repeat(seq(",", $._type)),
        ">",
      ),

    qualified_type: ($) =>
      seq($.identifier, ".", $.identifier),

    // ─── Literals & Identifiers ────────────────────────────────────

    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*/,

    integer: (_) => /[0-9][0-9_]*/,

    float: (_) => /[0-9][0-9_]*\.[0-9][0-9_]*/,

    string: (_) => seq('"', repeat(choice(/[^"\\]/, /\\./)), '"'),

    boolean: (_) => choice("true", "false"),

    self_: (_) => "self",

    _newlines: ($) => repeat1($._newline),
  },
});
