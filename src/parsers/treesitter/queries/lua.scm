; Global function declaration: function name() end
(function_declaration
  name: (identifier) @func_name)

; Local function declaration: local function name() end
(function_declaration
  name: (identifier) @local_func_name)

; Method declaration: function Class:method() end
(function_declaration
  name: (method_index_expression
    table: (identifier) @method_class
    method: (identifier) @method_name))

; Dot method declaration: function Class.method() end
(function_declaration
  name: (dot_index_expression
    table: (identifier) @dot_method_class
    field: (identifier) @dot_method_name))

; Local variable assignment: local x = value
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @local_var_name)
    (expression_list
      value: (_) @local_var_value)))

; Require call: local x = require("module")
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @require_alias)
    (expression_list
      value: (function_call
        name: (identifier) @_require_fn
        arguments: (arguments
          (string
            content: (string_content) @require_path))))))

; Module return: return M
(return_statement
  (expression_list
    (identifier) @module_return))
