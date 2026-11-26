use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HEADER: &str = "library sample version \"1.0.0\" abi_prefix sm;\n";

const C_OUTPUTS: &[&str] = &[
    "include/sample/sample.h",
    "sample.pc.in",
    "cmake/SampleConfig.cmake",
    "cmake/SampleConfigVersion.cmake",
];

const PYTHON_OUTPUTS: &[&str] = &[
    "pyproject.toml",
    "src/sample/py.typed",
    "src/sample/_runtime.py",
    "src/sample/_native.py",
    "src/sample/__init__.py",
];

const GO_OUTPUTS: &[&str] = &[
    "go.mod",
    "doc.go",
    "runtime.go",
    "callbacks.go",
    "sample.go",
];

const CSHARP_OUTPUTS: &[&str] = &[
    "Sample.csproj",
    "src/Runtime.cs",
    "src/Native.cs",
    "src/Sample.cs",
];

const JAVA_OUTPUTS: &[&str] = &[
    "pom.xml",
    "src/main/java/module-info.java",
    "src/main/java/os/sample/Sample.java",
    "src/main/java/os/sample/Native.java",
    "src/main/java/os/sample/Handle.java",
    "src/main/java/os/sample/Trampolines.java",
];

fn outputs(language: &str) -> &'static [&'static str] {
    match language {
        "c" => C_OUTPUTS,
        "cpp" => &["include/sample/sample.hpp"],
        "rust" => &["ffi.rs"],
        "python" => PYTHON_OUTPUTS,
        "go" => GO_OUTPUTS,
        "csharp" => CSHARP_OUTPUTS,
        "java" => JAVA_OUTPUTS,
        _ => panic!("no outputs listed for {language}"),
    }
}

const FOLDERS: &[&str] = &[
    "include/sample",
    "cmake",
    "src/sample",
    "src/main/java/os/sample",
];

fn scratch(name: &str) -> PathBuf {
    let root = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let folder = root.join(name);
    for inner in FOLDERS {
        std::fs::create_dir_all(folder.join(inner)).unwrap();
    }
    folder
}

fn generate(folder: &Path, idl: &str, language: &str, outputs: &[&str]) -> Output {
    let source = folder.join("sample.ridl");
    std::fs::write(&source, format!("{HEADER}{idl}\n")).unwrap();
    Command::new(std::env::var_os("RIKKA_SDK_CODEGEN").expect("RIKKA_SDK_CODEGEN is set"))
        .arg(&source)
        .arg(language)
        .args(outputs.iter().map(|output| folder.join(output)))
        .output()
        .unwrap()
}

#[test]
fn invalid_idl_is_reported_at_its_location() {
    let cases = [
        (
            "unknown_type",
            "c",
            "fn f(x: colour);",
            "sample.ridl:2:9: error: unknown type `colour`",
        ),
        (
            "missing_semicolon",
            "c",
            "fn f(x: u32)",
            "sample.ridl:3:1: error: expected `;`, found the end of the file",
        ),
        (
            "dangling_reference",
            "c",
            "/// See [missing].\nfn f();",
            "sample.ridl:2:9: error: `[missing]` refers to nothing",
        ),
        (
            "dangling_reference_in_tag",
            "c",
            "/// @param x See [missing].\nfn f(x: u32);",
            "sample.ridl:2:18: error: `[missing]` refers to nothing",
        ),
        (
            "unknown_parameter_doc",
            "c",
            "/// @param y Nope.\nfn f(x: u32);",
            "sample.ridl:2:12: error: there is no parameter `y`",
        ),
        (
            "shared_handed_over",
            "c",
            "shared class window {}\nfn f(window: owned window);",
            "sample.ridl:3:14: error: only an owned class can be handed over",
        ),
        (
            "owned_returned_borrowed",
            "c",
            "owned class a {}\nfn f() -> borrowed a;",
            "sample.ridl:3:11: error: only a string or a shared class can be returned borrowed",
        ),
        (
            "throws_plain_enum",
            "c",
            "enum e { a }\nfn f() throws e;",
            "sample.ridl:3:15: error: `e` is not an enum marked `@error`",
        ),
        (
            "c_keyword_parameter",
            "c",
            "fn f(class: u32);",
            "sample.ridl:2:6: error: `class` is a C or C++ keyword",
        ),
        (
            "c_keyword_receiver",
            "c",
            "owned class my_class { fn go(); }",
            "sample.ridl:2:27: error: `class` is a C or C++ keyword",
        ),
        (
            "c_builtin_type",
            "c",
            "struct str { a: u32 }",
            "sample.ridl:2:8: error: `sm_str` is declared twice in the C namespace",
        ),
        (
            "cpp_builtin_member",
            "cpp",
            "owned class thing { fn release(); }",
            "sample.ridl:2:24: error: this becomes the C++ member `release`",
        ),
        (
            "rust_generated_constant",
            "rust",
            "const version: u32 = 1;",
            "sample.ridl:2:7: error: this becomes the Rust constant `VERSION`, which is taken",
        ),
        (
            "python_named_constructor",
            "python",
            "owned class thing { constructor named(); fn from_named(); }",
            "sample.ridl:2:45: error: this becomes the Python member `from_named`",
        ),
        (
            "go_constant_and_value",
            "go",
            "const align_start: u32 = 1;\nenum align { start }",
            "sample.ridl:3:14: error: this becomes the Go name `AlignStart`, which is taken",
        ),
        (
            "csharp_member_names_class",
            "csharp",
            "owned class thing { fn thing(); }",
            "sample.ridl:2:24: error: this becomes the C# member `Thing`, which names its own class",
        ),
        (
            "java_closeable_method",
            "java",
            "owned class thing { fn close(); }",
            "sample.ridl:2:24: error: this becomes the Java method `close`",
        ),
    ];
    for (name, language, idl, expected) in cases {
        let folder = scratch(name);
        let output = generate(&folder, idl, language, outputs(language));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{name}: accepted invalid IDL");
        assert!(
            stderr.contains(expected),
            "{name}: expected `{expected}` in:\n{stderr}"
        );
    }
}

#[test]
fn every_output_must_be_declared() {
    let folder = scratch("undeclared");
    let output = generate(&folder, "fn f();", "c", &["sample.pc.in"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("produces `include/sample/sample.h`, but no output path names it"),
        "{stderr}"
    );
}

#[test]
fn outputs_land_at_their_declared_paths() {
    let folder = scratch("generated");
    let idl = "/// Adds.\n@thread_safe\nfn add(left: u32, right: u32) -> u32;";
    let output = generate(&folder, idl, "c", C_OUTPUTS);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(folder.join(C_OUTPUTS[0])).unwrap();
    assert!(header.contains("SM_API uint32_t sm_add(uint32_t left, uint32_t right);"));
    let pkgconfig = std::fs::read_to_string(folder.join(C_OUTPUTS[1])).unwrap();
    assert!(pkgconfig.contains("Version: 1.0.0"));
}

#[test]
fn every_language_projects_a_free_function() {
    let idl = "/// Adds.\n@thread_safe\nfn add(left: u32, right: u32) -> u32;";
    let cases = [
        (
            "python",
            "src/sample/__init__.py",
            "def add(left: int, right: int) -> int:",
        ),
        (
            "go",
            "sample.go",
            "func Add(left uint32, right uint32) uint32 {",
        ),
        (
            "csharp",
            "src/Sample.cs",
            "public static uint Add(uint left, uint right)",
        ),
        (
            "java",
            "src/main/java/os/sample/Sample.java",
            "public static int add(int left, int right) {",
        ),
    ];
    for (language, file, expected) in cases {
        let folder = scratch(&format!("projected_{language}"));
        let output = generate(&folder, idl, language, outputs(language));
        assert!(
            output.status.success(),
            "{language}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let source = std::fs::read_to_string(folder.join(file)).unwrap();
        assert!(
            source.contains(expected),
            "{language}: no `{expected}` in:\n{source}"
        );
    }
}
