; Package declaration
(package_clause (package_identifier) @package)

; Single import
(import_declaration
  (import_spec
    name: (package_identifier)? @import_alias
    path: (interpreted_string_literal) @import_path))

; Import block
(import_declaration
  (import_spec_list
    (import_spec
      name: (package_identifier)? @import_alias
      path: (interpreted_string_literal) @import_path)))

; Struct type
(type_declaration
  (type_spec
    name: (type_identifier) @struct_name
    type: (struct_type)))

; Interface type
(type_declaration
  (type_spec
    name: (type_identifier) @interface_name
    type: (interface_type)))

; Type alias (maps to type_identifier target — not struct/interface)
(type_declaration
  (type_spec
    name: (type_identifier) @type_alias_name
    type: (type_identifier) @type_alias_target))

; Standalone functions
(function_declaration
  name: (identifier) @func_name)

; Methods with pointer receiver
(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: (pointer_type (type_identifier) @method_receiver)))
  name: (field_identifier) @method_name)

; Methods with value receiver
(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: (type_identifier) @method_receiver_value))
  name: (field_identifier) @method_name_value)

; Constants (single and block)
(const_declaration
  (const_spec
    name: (identifier) @const_name))

; Package-level vars
(source_file
  (var_declaration
    (var_spec
      name: (identifier) @var_name)))
