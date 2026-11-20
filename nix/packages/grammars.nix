{
  lib,
  stdenv,
  fetchurl,
  symlinkJoin,
}:

let
  locked = (builtins.fromTOML (builtins.readFile ../../third_party/rust/Cargo.lock)).package;

  crate =
    name:
    let
      found = builtins.filter (package: package.name == name) locked;
      package = builtins.head found;
    in
    assert lib.assertMsg (
      builtins.length found == 1
    ) "expected exactly one ${name} in Cargo.lock, found ${toString (builtins.length found)}";
    {
      inherit (package) version;
      src = fetchurl {
        name = "${name}-${package.version}.tar.gz";
        url = "https://static.crates.io/crates/${name}/${name}-${package.version}.crate";
        sha256 = package.checksum;
      };
    };

  grammars = {
    bash = [
      "tree-sitter-bash"
      "src"
    ];
    c = [
      "tree-sitter-c"
      "src"
    ];
    cpp = [
      "tree-sitter-cpp"
      "src"
    ];
    csharp = [
      "tree-sitter-c-sharp"
      "src"
    ];
    css = [
      "tree-sitter-css"
      "src"
    ];
    diff = [
      "tree-sitter-diff"
      "src"
    ];
    go = [
      "tree-sitter-go"
      "src"
    ];
    html = [
      "tree-sitter-html"
      "src"
    ];
    java = [
      "tree-sitter-java"
      "src"
    ];
    javascript = [
      "tree-sitter-javascript"
      "src"
    ];
    json = [
      "tree-sitter-json"
      "src"
    ];
    lua = [
      "tree-sitter-lua"
      "src"
    ];
    markdown = [
      "tree-sitter-md"
      "tree-sitter-markdown/src"
    ];
    markdown_inline = [
      "tree-sitter-md"
      "tree-sitter-markdown-inline/src"
    ];
    nix = [
      "tree-sitter-nix"
      "src"
    ];
    php = [
      "tree-sitter-php"
      "php/src"
    ];
    python = [
      "tree-sitter-python"
      "src"
    ];
    ruby = [
      "tree-sitter-ruby"
      "src"
    ];
    rust = [
      "tree-sitter-rust"
      "src"
    ];
    slang = [
      "tree-sitter-slang"
      "src"
    ];
    sql = [
      "tree-sitter-sequel"
      "src"
    ];
    starlark = [
      "tree-sitter-starlark"
      "src"
    ];
    toml = [
      "tree-sitter-toml-ng"
      "src"
    ];
    tsx = [
      "tree-sitter-typescript"
      "tsx/src"
    ];
    typescript = [
      "tree-sitter-typescript"
      "typescript/src"
    ];
    yaml = [
      "tree-sitter-yaml"
      "src"
    ];
  };

  library =
    grammar: located:
    let
      name = builtins.elemAt located 0;
      source = builtins.elemAt located 1;
      pinned = crate name;
      file = "libtree-sitter-${grammar}.so";
    in
    stdenv.mkDerivation {
      pname = "tree-sitter-grammar-${grammar}";
      inherit (pinned) version src;

      env.NIX_DONT_SET_RPATH = true;

      buildPhase = ''
        runHook preBuild
        compiled=(${source}/parser.c)
        if [ -f ${source}/scanner.c ]; then
          compiled+=(${source}/scanner.c)
        fi
        $CC -shared -fPIC -std=c11 -O2 -w -I${source} "''${compiled[@]}" -o ${file}
        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall
        install -Dm755 ${file} $out/lib/rikka/grammars/${file}
        runHook postInstall
      '';
    };
in
symlinkJoin {
  name = "rikka-grammars";
  paths = lib.mapAttrsToList library grammars;
}
