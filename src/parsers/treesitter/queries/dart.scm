; Imports and exports
(import_or_export) @import_node

; Class definition
(class_definition
  name: (identifier) @class_name) @class_node

; Mixin declaration
(mixin_declaration) @mixin_node

; Extension declaration
(extension_declaration
  name: (identifier) @extension_name) @extension_node

; Extension type declaration
(extension_type_declaration
  name: (identifier) @extension_type_name) @extension_type_node

; Enum declaration
(enum_declaration
  name: (identifier) @enum_name) @enum_node

; Type alias (typedef)
(type_alias) @typedef_node

; Top-level function signature (with function body)
(program
  (function_signature
    name: (identifier) @top_func_name))

; Top-level function with body
(program
  (_
    (function_signature
      name: (identifier) @top_func_body_name)
    (function_body)))

; Getter signature at top level or in class
(getter_signature
  name: (identifier) @getter_name)

; Setter signature at top level or in class
(setter_signature
  name: (identifier) @setter_name)

; Constructor signature
(constructor_signature
  name: (identifier) @constructor_name)

; Factory constructor signature
(factory_constructor_signature
  (identifier) @factory_name)

; Constant constructor signature
(constant_constructor_signature
  (identifier) @const_constructor_name)

; Function signature inside declaration (methods)
(declaration
  (function_signature
    name: (identifier) @method_name))

; Static method
(declaration
  (function_signature
    name: (identifier) @static_method_name))

; Top-level final/const variable declarations
(program
  (initialized_identifier_list
    (initialized_identifier
      (identifier) @top_var_name)))

; Top-level static final declarations
(program
  (static_final_declaration_list
    (static_final_declaration
      (identifier) @top_const_name)))
