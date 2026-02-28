; Procedure declaration
(procedure_declaration
  name: (identifier) @proc_name)

; Function declaration
(function_declaration
  name: (identifier) @func_name)

; Module-level variable declaration
(source_file
  (var_declaration
    (var_name
      name: (identifier) @var_name)))

; Region
(region
  (region_start
    name: (identifier) @region_name))
