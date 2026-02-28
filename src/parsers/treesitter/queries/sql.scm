; CREATE TABLE — Class
(create_table_statement
  (identifier) @table_name)

; CREATE FUNCTION / CREATE PROCEDURE — Function
(create_function_statement
  (identifier) @func_name)

; CREATE INDEX — Property (uses field "name")
(create_index_statement
  name: (identifier) @index_name)

; CREATE TYPE — Class
(create_type_statement
  (identifier) @type_name)

; CREATE DOMAIN — Class
(create_domain_statement
  (identifier) @domain_name)
