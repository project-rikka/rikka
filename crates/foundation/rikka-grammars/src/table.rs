use crate::grammar::Entry;

pub(crate) static ENTRIES: [Entry; 26] = [
    Entry::new("bash", "tree_sitter_bash")
        .aliases(&["sh", "shell", "zsh"])
        .extensions(&["sh", "bash", "zsh", "ksh", "ash"])
        .filenames(&[".bashrc", ".bash_profile", ".profile", ".zshrc", "PKGBUILD"]),
    Entry::new("c", "tree_sitter_c").extensions(&["c", "h"]),
    Entry::new("cpp", "tree_sitter_cpp")
        .aliases(&["c++", "cxx", "cc"])
        .extensions(&[
            "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "inl",
        ]),
    Entry::new("csharp", "tree_sitter_c_sharp")
        .aliases(&["c#", "cs"])
        .extensions(&["cs", "csx"]),
    Entry::new("css", "tree_sitter_css").extensions(&["css"]),
    Entry::new("diff", "tree_sitter_diff")
        .aliases(&["patch", "udiff"])
        .extensions(&["diff", "patch"]),
    Entry::new("go", "tree_sitter_go")
        .aliases(&["golang"])
        .extensions(&["go"]),
    Entry::new("html", "tree_sitter_html")
        .aliases(&["htm", "xhtml"])
        .extensions(&["html", "htm", "xhtml"]),
    Entry::new("java", "tree_sitter_java").extensions(&["java"]),
    Entry::new("javascript", "tree_sitter_javascript")
        .aliases(&["js", "jsx", "node"])
        .extensions(&["js", "jsx", "mjs", "cjs"]),
    Entry::new("json", "tree_sitter_json")
        .aliases(&["jsonc"])
        .extensions(&["json", "jsonc", "webmanifest"]),
    Entry::new("lua", "tree_sitter_lua").extensions(&["lua"]),
    Entry::new("markdown", "tree_sitter_markdown")
        .aliases(&["md"])
        .extensions(&["md", "markdown", "mdown", "mkd"]),
    Entry::new("markdown_inline", "tree_sitter_markdown_inline"),
    Entry::new("nix", "tree_sitter_nix").extensions(&["nix"]),
    Entry::new("php", "tree_sitter_php").extensions(&["php", "phtml"]),
    Entry::new("python", "tree_sitter_python")
        .aliases(&["py"])
        .extensions(&["py", "pyi", "pyw"]),
    Entry::new("ruby", "tree_sitter_ruby")
        .aliases(&["rb"])
        .extensions(&["rb", "rake", "gemspec"])
        .filenames(&["Gemfile", "Rakefile"]),
    Entry::new("rust", "tree_sitter_rust")
        .aliases(&["rs"])
        .extensions(&["rs"]),
    Entry::new("slang", "tree_sitter_slang")
        .aliases(&["hlsl"])
        .extensions(&["slang", "hlsl", "hlsli"]),
    Entry::new("sql", "tree_sitter_sql")
        .aliases(&["sequel", "mysql", "postgresql", "sqlite"])
        .extensions(&["sql"]),
    Entry::new("starlark", "tree_sitter_starlark")
        .aliases(&["bazel", "bzl"])
        .extensions(&["bzl", "star", "bazel"])
        .filenames(&[
            "BUILD",
            "BUILD.bazel",
            "MODULE.bazel",
            "WORKSPACE",
            "WORKSPACE.bazel",
        ]),
    Entry::new("toml", "tree_sitter_toml").extensions(&["toml"]),
    Entry::new("tsx", "tree_sitter_tsx").extensions(&["tsx"]),
    Entry::new("typescript", "tree_sitter_typescript")
        .aliases(&["ts"])
        .extensions(&["ts", "mts", "cts"]),
    Entry::new("yaml", "tree_sitter_yaml")
        .aliases(&["yml"])
        .extensions(&["yaml", "yml"]),
];
