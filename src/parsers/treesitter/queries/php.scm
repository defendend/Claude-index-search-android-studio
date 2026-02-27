; Namespace definition
(namespace_definition
  name: (namespace_name) @namespace_name)

; Class declaration with optional extends and implements
(class_declaration
  name: (name) @class_name
  (base_clause (_) @class_parent)?
  (class_interface_clause (_) @class_interface)?)

; Interface declaration with optional extends
(interface_declaration
  name: (name) @interface_name
  (base_clause (_) @interface_parent)?)

; Trait declaration
(trait_declaration
  name: (name) @trait_name)

; Enum declaration
(enum_declaration
  name: (name) @enum_name)

; Function definition (top-level)
(function_definition
  name: (name) @func_name)

; Method declaration (inside class/trait/interface/enum)
(method_declaration
  name: (name) @method_name)

; Constant declaration (class-level or top-level)
(const_declaration
  (const_element
    (name) @const_name))

; Property declaration
(property_declaration
  (property_element
    (variable_name) @prop_name))

; Namespace use (imports) — use Foo\Bar; (qualified_name)
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @use_name))

; Namespace use (imports) — use Foo; (simple name)
(namespace_use_declaration
  (namespace_use_clause
    (name) @use_simple_name))

; Trait use inside class — use SomeTrait; (qualified_name)
(use_declaration
  (qualified_name) @trait_use_qualified)

; Trait use inside class — use SomeTrait; (simple name)
(use_declaration
  (name) @trait_use_name)
