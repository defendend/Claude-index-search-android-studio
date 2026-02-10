; Namespace declaration (block style)
(namespace_declaration
  name: (_) @namespace_name)

; File-scoped namespace declaration
(file_scoped_namespace_declaration
  name: (_) @namespace_name)

; Using directive (imports)
(using_directive) @using_dir

; Class declaration
(class_declaration
  name: (identifier) @class_name) @class_decl

; Interface declaration
(interface_declaration
  name: (identifier) @interface_name) @interface_decl

; Struct declaration
(struct_declaration
  name: (identifier) @struct_name)

; Record declaration
(record_declaration
  name: (identifier) @record_name) @record_decl

; Enum declaration
(enum_declaration
  name: (identifier) @enum_name)

; Method declaration
(method_declaration
  name: (identifier) @method_name)

; Constructor declaration
(constructor_declaration
  name: (identifier) @constructor_name)

; Property declaration
(property_declaration
  name: (identifier) @property_name)

; Field declaration
(field_declaration) @field_decl

; Event field declaration (event EventHandler OnData;)
(event_field_declaration) @event_field_decl

; Event declaration (event with accessors)
(event_declaration
  name: (identifier) @event_name)

; Delegate declaration
(delegate_declaration
  name: (identifier) @delegate_name)

; Attribute list
(attribute_list
  (attribute
    name: (_) @attr_name))
