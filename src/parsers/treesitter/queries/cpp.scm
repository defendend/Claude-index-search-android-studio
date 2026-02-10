; === Classes and Structs ===

; class Name { ... }
(class_specifier
  name: (type_identifier) @class_name
  body: (field_declaration_list)) @class_node

; struct Name { ... }
(struct_specifier
  name: (type_identifier) @struct_name
  body: (field_declaration_list)) @struct_node

; template<...> class/struct Name { ... }
(template_declaration
  (class_specifier
    name: (type_identifier) @template_class_name
    body: (field_declaration_list)) @template_class_node)

(template_declaration
  (struct_specifier
    name: (type_identifier) @template_struct_name
    body: (field_declaration_list)) @template_struct_node)

; === Functions ===

; Regular function definition at file/namespace scope
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @func_name))

; Template function
(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @template_func_name)))

; Method definition outside class: ReturnType ClassName::MethodName(...)
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      scope: (namespace_identifier) @method_class
      name: (identifier) @method_name)))

; Template method definition outside class
(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (qualified_identifier
        scope: (namespace_identifier) @template_method_class
        name: (identifier) @template_method_name))))

; Destructor definition outside class: ClassName::~ClassName()
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      scope: (namespace_identifier) @destructor_class
      name: (destructor_name) @destructor_name)))

; === Namespaces ===

; namespace Name { ... }
(namespace_definition
  name: (namespace_identifier) @namespace_name)

; === Enums ===

; enum Name { ... } or enum class Name { ... }
(enum_specifier
  name: (type_identifier) @enum_name)

; === Type Aliases ===

; typedef ... TypeName; (simple)
(type_definition
  declarator: (type_identifier) @typedef_name)

; typedef with function pointer: typedef void (*Callback)(int, int);
; Capture the whole type_definition node for complex declarators
(type_definition) @typedef_node

; using TypeName = ...;
(alias_declaration
  name: (type_identifier) @using_alias_name)

; === Macros ===

; #define MACRO(...) — function-like macro
(preproc_function_def
  name: (identifier) @macro_name)

; === Includes ===

; #include <path> or #include "path"
(preproc_include
  path: (_) @include_path)
