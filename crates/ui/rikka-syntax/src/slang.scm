(call_expression
  function: (qualified_identifier
    name: (identifier) @function))

(template_function
  name: (identifier) @function)

(template_method
  name: (field_identifier) @function)

(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))

(function_declarator
  declarator: (field_identifier) @function)

((namespace_identifier) @type
 (#match? @type "^[A-Z]"))

(this) @variable.builtin

[
  "class"
  "namespace"
  "private"
  "protected"
  "public"
  "template"
  "typename"
  "using"
  "override"
] @keyword

[
  "import"
  "__exported"
  "module"
  "interface"
  "extension"
  "associatedtype"
  "property"
  "get"
  "set"
  "let"
  "var"
  "where"
  "is"
  "as"
  "some"
  "dyn"
  "__init"
  "__subscript"
  "cbuffer"
  "discard"
] @keyword

[
  "in"
  "out"
  "inout"
  "uniform"
  "groupshared"
  "globallycoherent"
  "precise"
  "nointerpolation"
  "noperspective"
  "centroid"
  "linear"
  "sample"
  "row_major"
  "column_major"
  "snorm"
  "unorm"
  "point"
  "line"
  "lineadj"
  "triangle"
  "triangleadj"
] @keyword

"This" @type.builtin
