; Package declaration
(package_declaration
  (identifier) @package_name)
(package_declaration
  (scoped_identifier) @package_name)

; Import declarations
(import_declaration
  (identifier) @import_path)
(import_declaration
  (scoped_identifier) @import_path)

; Class declarations
(class_declaration
  name: (identifier) @class_name)

; Interface declarations
(interface_declaration
  name: (identifier) @interface_name)

; Enum declarations
(enum_declaration
  name: (identifier) @enum_name)

; Method declarations
(method_declaration
  name: (identifier) @method_name)

; Constructor declarations
(constructor_declaration
  name: (identifier) @constructor_name)

; Field declarations
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @field_name))
