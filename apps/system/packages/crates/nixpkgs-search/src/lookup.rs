use nixpkgs_catalog::Package;

use crate::cancel::Cancel;
use crate::error::Result;
use crate::meta;
use crate::nix::{self, installable, quoted};

const PROJECTION: &str = r#"pkgs:
let
  inherit (pkgs) lib;
  root = "${toString pkgs.path}/";
  systems = map lib.systems.elaborate lib.systems.doubles.all;
  present = value: builtins.filter (item: item != null) (lib.toList value);
  strings = value: builtins.filter builtins.isString (lib.toList value);
  licence = value:
    if builtins.isString value
    then { name = value; spdx = ""; url = ""; }
    else {
      name = value.fullName or value.shortName or "";
      spdx = value.spdxId or "";
      url = value.url or "";
    };
  person = value: {
    name = value.name or "";
    github = value.github or "";
    email = value.email or "";
  };
  team = value: {
    name = value.shortName or value.github or "";
    scope = value.scope or "";
  };
  reach = set: taken: parts:
    let
      whole = builtins.concatStringsSep "." parts;
      here = builtins.head parts;
      rest = builtins.tail parts;
    in
    if set ? ${whole} then { drv = set.${whole}; from = builtins.concatStringsSep "." taken; }
    else if rest == [ ] then null
    else if set ? ${here} && builtins.isAttrs set.${here} then reach set.${here} (taken ++ [ here ]) rest
    else null;
  read = attribute:
    let found = reach pkgs [ ] (lib.splitString "." attribute); in
    if found == null || !(lib.isDerivation found.drv) then null else
    let drv = found.drv; meta = drv.meta or { }; in {
      inherit attribute;
      set = found.from;
      name = lib.getName drv;
      version = lib.getVersion drv;
      description = meta.description or "";
      about = meta.longDescription or "";
      unfree = meta.unfree or false;
      main_program = meta.mainProgram or "";
      outputs = strings (drv.outputs or [ ]);
      platforms = map (system: system.system)
        (builtins.filter (system: lib.meta.availableOn system drv) systems);
      licences = map licence (present (meta.license or [ ]));
      maintainers = map person (present (meta.maintainers or [ ]));
      teams = map team (present (meta.teams or [ ]));
      homepages = strings (meta.homepage or [ ]);
      position = lib.removePrefix root (meta.position or "");
    };
  settled = attribute:
    let answered = builtins.tryEval (let found = read attribute; in builtins.deepSeq found found);
    in if answered.success then answered.value else null;
in
  present (map settled "#;

pub(crate) fn packages(system: &str, attributes: &[&str], cancel: &Cancel) -> Result<Vec<Package>> {
    if attributes.is_empty() {
        return Ok(Vec::new());
    }
    let answered = nix::run(
        &[
            "eval",
            &installable(system),
            "--json",
            "--apply",
            &projection(attributes),
        ],
        cancel,
    )?;

    meta::read(&answered)
}

fn projection(attributes: &[&str]) -> String {
    let listed: Vec<String> = attributes.iter().map(|name| quoted(name)).collect();
    format!("{PROJECTION}[ {} ])", listed.join(" "))
}
