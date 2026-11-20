use rikka_slang::{
    Argument, Binding, Compiler, ParameterSchema, Scalar, SchemaError, Shape, Source, Type,
    TypeLayout,
};

const MATERIAL: &str = r#"
[__AttributeUsage(_AttributeTargets.Var)]
struct RangeAttribute { float low; float high; };

[__AttributeUsage(_AttributeTargets.Var)]
struct GroupAttribute { string name; };

[__AttributeUsage(_AttributeTargets.Var)]
struct StepsAttribute { int count; };

struct Lobe {
    float3 tint;
    float weight;
};

struct Material {
    [Range(0.0, 1.0)] [Group("surface")] float roughness;
    float3 albedo;
    [Steps(4)] uint bands;
    Lobe lobes[2];
    float4x4 frame;
};

struct Push {
    Material* material;
    uint slot;
};

[[vk::push_constant]] ConstantBuffer<Push> push;
[[vk::binding(0, 0)]] Sampler2D textures[];
[[vk::binding(2, 0)]] SamplerState samplers[];

[shader("fragment")]
float4 shade(float4 position : SV_Position) : SV_Target {
    Material material = *push.material;
    float4 sampled = textures[push.slot].Sample(position.xy);
    float4 other = textures[push.slot].Sample(position.xy);
    return sampled * material.roughness + float4(material.albedo, 1.0)
        + float4(material.lobes[1].tint * material.lobes[0].weight, float(material.bands))
        + mul(material.frame, other);
}
"#;

fn field<'a>(layout: &'a TypeLayout, name: &str) -> &'a rikka_slang::Field {
    layout
        .ty
        .fields()
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("no field {name} in {layout:?}"))
}

#[test]
fn struct_layout_follows_scalar_rules_and_keeps_attributes() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(&Source::new("material", MATERIAL).layout_of("Material"))
        .expect("compiles");
    let material = program.type_layout("Material").expect("Material layout");

    let offsets: Vec<_> = material
        .ty
        .fields()
        .iter()
        .map(|field| (field.name.as_str(), field.offset))
        .collect();
    assert_eq!(
        offsets,
        [
            ("roughness", 0),
            ("albedo", 4),
            ("bands", 16),
            ("lobes", 20),
            ("frame", 52),
        ]
    );
    assert_eq!(material.size, 116);

    let roughness = field(material, "roughness");
    assert_eq!(roughness.layout.ty, Type::Scalar(Scalar::F32));
    let range = roughness.attribute("Range").expect("Range attribute");
    assert_eq!(
        range.arguments,
        [Argument::Float(0.0), Argument::Float(1.0)]
    );
    let group = roughness.attribute("Group").expect("Group attribute");
    assert_eq!(group.arguments[0].as_str(), Some("surface"));
    let steps = field(material, "bands").attribute("Steps").expect("Steps");
    assert_eq!(steps.arguments, [Argument::Int(4)]);

    let Type::Array {
        element,
        count,
        stride,
    } = &field(material, "lobes").layout.ty
    else {
        panic!("lobes is an array");
    };
    assert_eq!((*count, *stride), (Some(2), 16));
    assert_eq!(field(element, "weight").offset, 12);
    assert_eq!(
        field(material, "frame").layout.ty,
        Type::Matrix {
            scalar: Scalar::F32,
            rows: 4,
            columns: 4
        }
    );
}

#[test]
fn global_parameters_report_bindings() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(&Source::new("material", MATERIAL))
        .expect("compiles");

    let push = program.parameter("push").expect("push constants");
    assert_eq!(push.binding, Binding::PushConstant);
    let Type::ConstantBuffer(block) = &push.layout.ty else {
        panic!("push is a constant buffer: {:?}", push.layout);
    };
    assert_eq!(
        field(block, "material").layout.ty,
        Type::Pointer {
            pointee: "Material".into()
        }
    );
    assert_eq!(field(block, "slot").offset, 8);

    let textures = program.parameter("textures").expect("texture table");
    assert_eq!(textures.binding, Binding::Descriptor { set: 0, binding: 0 });
    let Type::Array { element, count, .. } = &textures.layout.ty else {
        panic!("textures is an array: {:?}", textures.layout);
    };
    assert_eq!(*count, None);
    assert!(matches!(
        element.ty,
        Type::Resource {
            shape: Shape::Texture2d,
            ..
        }
    ));
}

const SCHEMA_ATTRIBUTES: &str = r#"
[__AttributeUsage(_AttributeTargets.Var)]
struct ColorAttribute {};
[__AttributeUsage(_AttributeTargets.Var)]
struct SrgbAttribute {};
[__AttributeUsage(_AttributeTargets.Var)]
struct OptionsAttribute { string names; };
[__AttributeUsage(_AttributeTargets.Var)]
struct DefaultAttribute { float x; float y = 0.0; float z = 0.0; float w = 0.0; };
"#;

type Expected = fn(&SchemaError) -> bool;

#[test]
fn schema_refuses_parameters_it_cannot_describe() {
    let compiler = Compiler::new().expect("libslang loads");
    let cases: [(&str, Expected); 4] = [
        (
            "float4x4 frame;",
            |error| matches!(error, SchemaError::Unsupported { name, .. } if name == "frame"),
        ),
        (
            "[Color] float tint;",
            |error| matches!(error, SchemaError::Attribute { name, .. } if name == "tint"),
        ),
        (
            "[Srgb] float3 tint;",
            |error| matches!(error, SchemaError::Attribute { name, .. } if name == "tint"),
        ),
        (
            "[Options(\"a|b\")] [Default(2)] uint mode;",
            |error| matches!(error, SchemaError::Attribute { name, .. } if name == "mode"),
        ),
    ];
    for (declaration, expected) in cases {
        let source = format!(
            "{SCHEMA_ATTRIBUTES}\nstruct Block {{ {declaration} }};\n\
             struct Push {{ Block* block; }};\n\
             [[vk::push_constant]] ConstantBuffer<Push> push;\n\
             [shader(\"fragment\")] float4 shade() : SV_Target {{ return float4(0.0); }}\n"
        );
        let program = compiler
            .compile(&Source::new("schema", &source).layout_of("Block"))
            .expect("compiles");
        let layout = program.type_layout("Block").expect("Block layout");
        match ParameterSchema::from_struct(layout, 0) {
            Err(error) if expected(&error) => {}
            other => panic!("{declaration}: unexpected {other:?}"),
        }
    }
}
