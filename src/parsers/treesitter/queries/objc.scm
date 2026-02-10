; ObjC class interface: @interface ClassName : SuperClass <Proto1, Proto2>
(class_interface) @class_interface

; ObjC protocol declaration: @protocol Name <ParentProto>
(protocol_declaration) @protocol_decl

; ObjC class implementation: @implementation ClassName
(class_implementation) @class_impl

; ObjC method declaration (in @interface or @protocol)
(method_declaration) @method_decl

; ObjC method definition (in @implementation, with body)
(method_definition) @method_def

; ObjC property: @property (attrs) Type *name;
(property_declaration) @property_decl

; C typedef (common in ObjC headers): typedef struct { ... } TypeName;
(type_definition) @typedef_decl
