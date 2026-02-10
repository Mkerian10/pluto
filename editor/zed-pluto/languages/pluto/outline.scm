(function_definition
  "fn"
  name: (identifier) @name) @item

(class_definition
  "class"
  name: (identifier) @name) @item

(trait_definition
  "trait"
  name: (identifier) @name) @item

(enum_definition
  "enum"
  name: (identifier) @name) @item

(error_definition
  "error"
  name: (identifier) @name) @item

(app_definition
  "app"
  name: (identifier) @name) @item

(test_definition
  "test"
  name: (string) @name) @item
