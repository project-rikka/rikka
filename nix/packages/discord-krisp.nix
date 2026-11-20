{
  fetchurl,
  runCommand,
  brotli,
  gnutar,
}:

let
  release = "1.0.167";
  module = "1";
  package = fetchurl {
    name = "discord-krisp-${release}-${module}.distro";
    url = "https://stable.dl2.discordapp.net/distro/app/stable/linux/x64/${release}/discord_krisp/${module}/full.distro";
    hash = "sha256-91F1w9mjK+LEhM9OE4ml8GJJYoQ1U1C7aYCOZNnsEWI=";
  };
in
runCommand "discord-krisp-${release}-${module}"
  {
    nativeBuildInputs = [
      brotli
      gnutar
    ];
  }
  ''
    mkdir -p $out/lib/discord_krisp/${release}-${module}
    brotli -dc ${package} | tar -x -C $out/lib/discord_krisp/${release}-${module} --strip-components=1 --wildcards 'files/discord_krisp.node' 'files/*.kef'
  ''
