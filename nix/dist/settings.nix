{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (lib) mkOption types;

  cfg = config.rikka;

  catalog = lib.importJSON ./settings.json;

  toml = pkgs.formats.toml { };

  value =
    types.oneOf [
      types.bool
      types.int
      types.float
      types.str
      (types.listOf value)
      (types.attrsOf value)
    ]
    // {
      description = "TOML value";
    };

  preferences = entry: lib.filter (key: (key.role or "preference") == "preference") entry.keys;

  declarable = lib.filter (entry: preferences entry != [ ]) catalog.domains;

  entries = lib.listToAttrs (map (entry: lib.nameValuePair entry.domain entry) catalog.domains);

  keyType =
    key:
    let
      kind = builtins.typeOf key.default;
      constraint = key.constraint or { };
    in
    if constraint ? choices then
      types.enum constraint.choices
    else if constraint ? range then
      (if kind == "int" then types.ints.between else types.numbers.between) constraint.range.min
        constraint.range.max
    else
      {
        bool = types.bool;
        int = types.int;
        float = types.number;
        string = types.str;
        list = types.listOf value;
        set = types.attrsOf value;
      }
      .${kind};

  keyOption =
    entry: key:
    mkOption {
      type = types.nullOr (keyType key);
      default = null;
      description = ''
        ${key.summary or "The `${key.name}` setting of `${entry.domain}`."}

        `null` leaves it to Rikka, which uses `${builtins.toJSON key.default}` unless the
        person chooses otherwise.
      '';
    };

  keyOptions =
    entry:
    lib.listToAttrs (map (key: lib.nameValuePair key.name (keyOption entry key)) (preferences entry));

  domainOption =
    entry:
    mkOption {
      type =
        if entry ? instances then
          types.attrsOf (types.submodule { options = keyOptions entry; })
        else
          types.submodule { options = keyOptions entry; };
      default = { };
      description =
        "${entry.summary or entry.domain}."
        + lib.optionalString (entry ? instances) " Each attribute names one ${entry.instances}.";
    };

  settingsType = types.submodule {
    freeformType = types.attrsOf (types.attrsOf value);
    options = lib.listToAttrs (
      map (entry: lib.nameValuePair entry.domain (domainOption entry)) declarable
    );
  };

  lockedType = types.submodule {
    freeformType = types.attrsOf (types.listOf types.str);
    options = lib.listToAttrs (
      map (
        entry:
        lib.nameValuePair entry.domain (mkOption {
          type = types.listOf (
            if entry ? instances then types.str else types.enum (map (key: key.name) (preferences entry))
          );
          default = [ ];
          description = "Settings of `${entry.domain}` that are kept at their declared value.";
        })
      ) declarable
    );
  };

  scopeOptions = whom: {
    settings = mkOption {
      type = settingsType;
      default = { };
      example = lib.literalExpression ''
        {
          "rikka.windows".layout = "tiling";
          "rikka.pointer".touchpad-tap = true;
          "rikka.displays"."dell-u2720q-1a2b3c".scale = 1.5;
        }
      '';
      description = ''
        Rikka settings declared for ${whom}, by domain. They take the place of Rikka's own
        defaults: Settings and the applications still change them, and resetting a setting
        brings back the value declared here. Displays are named as the `rikka.displays` domain
        names them, and their settings are nested under that name.
      '';
    };

    locked = mkOption {
      type = lockedType;
      default = { };
      example = lib.literalExpression ''
        {
          "rikka.windows" = [ "layout" ];
        }
      '';
      description = ''
        Settings, by domain, that stay at the value declared for ${whom} in the `settings`
        beside this option: Settings and the applications show them locked and cannot change
        them. Settings of displays are named `<display>-<setting>`.
      '';
    };
  };

  flatten =
    domain: values:
    let
      declared = lib.filterAttrs (_: setting: setting != null);
    in
    if entries ? ${domain} && entries.${domain} ? instances then
      lib.concatMapAttrs (
        instance: settings:
        lib.mapAttrs' (suffix: lib.nameValuePair "${instance}-${suffix}") (declared settings)
      ) values
    else
      declared values;

  unlockable =
    scope:
    lib.concatLists (
      lib.mapAttrsToList (
        domain: names:
        let
          declared = flatten domain (scope.settings.${domain} or { });
        in
        map (name: "${domain}/${name}") (lib.filter (name: !(declared ? ${name})) names)
      ) scope.locked
    );

  files =
    folder: scope:
    lib.concatMapAttrs (
      domain: values:
      let
        declared = flatten domain values;
        locked = lib.filter (name: declared ? ${name}) (scope.locked.${domain} or [ ]);
        free = removeAttrs declared locked;
        kept = lib.getAttrs locked declared;
      in
      lib.optionalAttrs (free != { }) {
        "rikka/settings/${folder}/${domain}.toml".source = toml.generate "${domain}.toml" free;
      }
      // lib.optionalAttrs (kept != { }) {
        "rikka/settings/${folder}/locked/${domain}.toml".source = toml.generate "${domain}.toml" kept;
      }
    ) scope.settings;
in
{
  options.rikka = scopeOptions "every user" // {
    users = mkOption {
      type = types.attrsOf (types.submodule { options = scopeOptions "this user"; });
      default = { };
      example = lib.literalExpression ''
        {
          alice.settings."rikka.appearance".taskbar-position = "top";
        }
      '';
      description = ''
        Rikka settings declared for one user, by user name, over those declared for every user.
        Rikka's Settings saves what the person chose here, in the `settings` module.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions =
      map (setting: {
        assertion = false;
        message = "rikka.locked locks ${setting}, which rikka.settings does not declare.";
      }) (unlockable cfg)
      ++ lib.concatLists (
        lib.mapAttrsToList (
          name: user:
          [
            {
              assertion = config.users.mutableUsers || lib.hasAttr name config.users.users;
              message = "rikka.users.${name} declares settings for ${name}, who is not in users.users, and users.mutableUsers is off.";
            }
          ]
          ++ map (setting: {
            assertion = false;
            message = "rikka.users.${name}.locked locks ${setting}, which rikka.users.${name}.settings does not declare.";
          }) (unlockable user)
        ) cfg.users
      );

    environment.etc =
      files "machine" cfg // lib.concatMapAttrs (name: user: files "users/${name}" user) cfg.users;
  };
}
