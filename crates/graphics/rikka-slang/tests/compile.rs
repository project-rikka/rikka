use rikka_slang::{Compiler, Error, Optimization, Source, Stage};

const SPIRV_MAGIC: u32 = 0x0723_0203;

const PROGRAM: &str = r#"
[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] RWTexture2D<float> target[];

[shader("compute")]
[numthreads(8, 4, 1)]
void fill(uint3 id : SV_DispatchThreadID) {
    target[0][id.xy] = float(id.x);
}

[shader("fragment")]
float4 shade(float4 position : SV_Position) : SV_Target {
    return float4(position.xy, 0.0, 1.0);
}
"#;

#[test]
fn module_compiles_each_entry_point_to_spirv() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(&Source::new("program", PROGRAM))
        .expect("compiles");

    let stages: Vec<_> = program
        .entry_points()
        .iter()
        .map(|entry| (entry.name.as_str(), entry.stage))
        .collect();
    assert_eq!(
        stages,
        [("fill", Stage::Compute), ("shade", Stage::Fragment)]
    );
    let fill = program.entry_point("fill").expect("fill entry");
    assert_eq!(fill.workgroup_size, Some([8, 4, 1]));
    for entry in program.entry_points() {
        assert_eq!(entry.spirv[0], SPIRV_MAGIC, "{} is SPIR-V", entry.name);
    }
}

#[test]
fn chosen_entry_points_are_compiled_alone() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(&Source::new("program", PROGRAM).entry("shade"))
        .expect("compiles");
    assert_eq!(program.entry_points().len(), 1);
    assert_eq!(program.entry_points()[0].stage, Stage::Fragment);

    let missing = compiler.compile(&Source::new("program", PROGRAM).entry("absent"));
    assert!(matches!(missing, Err(Error::MissingEntryPoint(name)) if name == "absent"));
}

#[test]
fn compile_error_reports_diagnostics() {
    let compiler = Compiler::new().expect("libslang loads");
    let error = compiler
        .compile(&Source::new(
            "broken",
            "[shader(\"fragment\")] float4 shade() : SV_Target { return undeclared_name; }",
        ))
        .expect_err("undeclared name fails");
    let Error::Compile { diagnostics } = &error else {
        panic!("expected a compile error, got {error}");
    };
    assert!(diagnostics.contains("undeclared_name"), "{diagnostics}");
    assert!(
        error.to_string().starts_with("failed to compile shader: "),
        "{error}"
    );
}

const ARITHMETIC: &str = r#"
[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] RWTexture2D<float> target[];

[shader("compute")]
[numthreads(8, 8, 1)]
void blend(uint3 id : SV_DispatchThreadID) {
    float a = float(id.x);
    float b = float(id.y);
    target[0][id.xy] = a * b + (a - b) * 0.5;
}
"#;

const OP_DECORATE: u32 = 71;
const DECORATION_NO_CONTRACTION: u32 = 42;
const OP_FADD: u32 = 129;
const OP_FSUB: u32 = 131;
const OP_FMUL: u32 = 133;

const OP_FUNCTION: u32 = 54;

struct Scanned {
    functions: usize,
    contracted: Vec<u32>,
    arithmetic: usize,
}

fn scan(words: &[u32]) -> Scanned {
    let mut uncontracted = Vec::new();
    let mut arithmetic = Vec::new();
    let mut functions = 0;
    let mut at = 5;
    while at < words.len() {
        let opcode = words[at] & 0xffff;
        let count = (words[at] >> 16) as usize;
        match opcode {
            OP_DECORATE if words[at + 2] == DECORATION_NO_CONTRACTION => {
                uncontracted.push(words[at + 1]);
            }
            OP_FADD | OP_FSUB | OP_FMUL => arithmetic.push(words[at + 2]),
            OP_FUNCTION => functions += 1,
            _ => {}
        }
        at += count;
    }
    Scanned {
        functions,
        arithmetic: arithmetic.len(),
        contracted: arithmetic
            .into_iter()
            .filter(|result| !uncontracted.contains(result))
            .collect(),
    }
}

#[test]
fn float_arithmetic_is_never_contracted() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(&Source::new("arithmetic", ARITHMETIC))
        .expect("compiles");
    let scanned = scan(&program.entry_point("blend").expect("blend entry").spirv);
    assert!(scanned.arithmetic > 0, "the program does float arithmetic");
    assert!(
        scanned.contracted.is_empty(),
        "float operations {:?} may be fused by the driver",
        scanned.contracted
    );
}

const CALLS: &str = r#"
[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] RWTexture2D<float> target[];

float mix(float a, float b) {
    if (a > b) {
        return a * b + (a - b) * 0.5;
    }
    return a - b;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void blend(uint3 id : SV_DispatchThreadID) {
    target[0][id.xy] = mix(float(id.x), float(id.y)) + mix(float(id.y), 2.0);
}
"#;

#[test]
fn flattened_entries_inline_every_call_and_stay_uncontracted() {
    let compiler = Compiler::builder()
        .optimization(Optimization::None)
        .build()
        .expect("libslang loads");
    let entry = |source: Source<'_>| {
        let program = compiler.compile(&source).expect("compiles");
        scan(&program.entry_point("blend").expect("blend entry").spirv)
    };
    let called = entry(Source::new("calls", CALLS));
    assert!(called.functions > 1, "unoptimized code keeps its calls");

    let flattened = entry(Source::new("calls", CALLS).flatten());
    assert_eq!(flattened.functions, 1, "every call is inlined");
    assert!(
        flattened.arithmetic > 0,
        "the program does float arithmetic"
    );
    assert!(
        flattened.contracted.is_empty(),
        "inlined float operations {:?} may be fused by the driver",
        flattened.contracted
    );
}

#[test]
fn embedded_modules_are_importable() {
    let compiler = Compiler::builder()
        .modules(&[
            ("toolkit", "__exported import toolkit.colour;\n"),
            (
                "toolkit.colour",
                "import toolkit.colour.weights;\n\
                 float luma(float3 colour) { return dot(colour, rec709()); }\n",
            ),
            (
                "toolkit.colour.weights",
                "float3 rec709() { return float3(0.2126, 0.7152, 0.0722); }\n",
            ),
        ])
        .build()
        .expect("libslang loads");
    let program = compiler
        .compile(&Source::new(
            "user",
            "import toolkit;\n\
             [shader(\"fragment\")] float4 shade(float4 p : SV_Position) : SV_Target \
             { return float4(luma(p.xyz)); }\n",
        ))
        .expect("imports the embedded module");
    assert_eq!(program.entry_points()[0].spirv[0], SPIRV_MAGIC);

    let unknown = compiler.compile(&Source::new(
        "user",
        "import missing_module;\n[shader(\"fragment\")] float4 shade() : SV_Target { return 0; }\n",
    ));
    assert!(matches!(unknown, Err(Error::Compile { .. })), "{unknown:?}");
}

#[test]
fn absent_requested_types_have_no_layout() {
    let compiler = Compiler::new().expect("libslang loads");
    let program = compiler
        .compile(
            &Source::new("program", PROGRAM)
                .entry("fill")
                .layout_of("Absent"),
        )
        .expect("an absent requested type is not an error");
    assert!(program.type_layout("Absent").is_none());
    assert_eq!(program.entry_points().len(), 1);
}

#[test]
fn concurrent_compiles_share_one_compiler() {
    let compiler = Compiler::new().expect("libslang loads");
    let programs: Vec<_> = std::thread::scope(|scope| {
        let running: Vec<_> = (0..12)
            .map(|index| {
                let compiler = &compiler;
                scope.spawn(move || {
                    let text = format!("{PROGRAM}\nstatic const float VERSION = {index}.0;\n");
                    compiler.compile(&Source::new("concurrent", &text))
                })
            })
            .collect();
        running
            .into_iter()
            .map(|compile| compile.join().expect("compile thread"))
            .collect()
    });
    for program in programs {
        let program = program.expect("compiles");
        let names: Vec<_> = program
            .entry_points()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, ["fill", "shade"]);
    }
}

const RECOMPILES: usize = 96;

fn resident_bytes() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").expect("procfs");
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|field| field.parse().ok())
        .expect("statm lists resident pages");
    pages * 4096
}

#[test]
fn recompiling_keeps_memory_bounded() {
    let compiler = Compiler::new().expect("libslang loads");
    let compile = |index: usize| {
        let text = format!("{PROGRAM}\nstatic const float VERSION = {index}.0;\n");
        compiler
            .compile(&Source::new("reloaded", &text).entry("shade"))
            .expect("compiles");
    };
    (0..RECOMPILES / 4).for_each(compile);
    let settled = resident_bytes();
    (RECOMPILES / 4..RECOMPILES).for_each(compile);
    let grown = resident_bytes().saturating_sub(settled);
    assert!(grown < 8 << 20, "{grown} bytes grew over many recompiles");
}
