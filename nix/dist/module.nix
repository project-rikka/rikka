{
  release,
  packages,
  chrome,
}:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.rikka;

  inherit (release) components;

  optional = lib.filterAttrs (_: entry: entry.offer != "required") components;

  chosen = lib.attrNames (
    lib.filterAttrs (id: entry: entry.offer == "required" || cfg.apps.${id}.enable) components
  );

  installed = map (id: packages.${id}) chosen;

  base = packages.base;

  extensions = lib.concatMap (
    id: map (extension: extension // { package = packages.${id}; }) components.${id}.extensions
  ) chosen;

  updateManifest = extension: "rikka/web/updates/${extension.id}.xml";

  updateResponse =
    extension:
    pkgs.writeText "rikka-chrome-${extension.id}.xml" ''
      <?xml version="1.0" encoding="UTF-8"?>
      <gupdate xmlns="http://www.google.com/update2/response" protocol="2.0">
        <app appid="${extension.id}">
          <updatecheck codebase="file://${extension.package}/${extension.crx}" version="${extension.version}"/>
        </app>
      </gupdate>
    '';

  pamHelper = pkgs.stdenv.mkDerivation {
    pname = "rikka-pam-helper";
    inherit (base) version;
    src = base;
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    buildInputs = [
      pkgs.pam
      pkgs.stdenv.cc.cc.lib
    ];
    dontConfigure = true;
    dontBuild = true;
    installPhase = ''
      runHook preInstall
      install -Dm755 bin/rikka-pam-helper "$out/bin/rikka-pam-helper"
      runHook postInstall
    '';
  };

  autologin = "rikka-autologin";

  console = "getty@tty1.service";

  overNixosDefault = lib.mkOverride 900;

  absolutePath = lib.types.strMatching "(/(\\.?[^./][^/]*|\\.\\.[^/]+))+";

  otherDisplayManagers = lib.attrNames (
    lib.filterAttrs (
      name: service:
      name != autologin
      && service.enable
      && (name == "display-manager" || lib.elem "display-manager.service" service.aliases)
    ) config.systemd.services
  );
in
{
  imports = [ ./settings.nix ];

  options.rikka = {
    enable = lib.mkEnableOption "the Rikka desktop";

    apps = lib.mapAttrs (_: entry: {
      enable = lib.mkEnableOption "${entry.name}: ${entry.summary}";
    }) optional;

    autologin.user = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "alice";
      description = "Starts Rikka on tty1 as this user at boot, without a greeter.";
    };

    configuration = {
      flake = lib.mkOption {
        type = lib.types.nullOr absolutePath;
        default = "/etc/nixos";
        example = "/home/alice/nixos";
        description = ''
          The flake this system is built from. Rikka's Settings, Packages and system updates
          edit the modules in {option}`rikka.configuration.modules` and rebuild this flake as
          root. `null` turns those features off.
        '';
      };

      modules = lib.mkOption {
        type = lib.types.nullOr absolutePath;
        default = if cfg.configuration.flake == null then null else "${cfg.configuration.flake}/rikka";
        defaultText = lib.literalExpression ''
          if config.rikka.configuration.flake == null then null else "''${config.rikka.configuration.flake}/rikka"
        '';
        example = "/home/alice/nixos/hosts/laptop/rikka";
        description = ''
          The directory inside the flake holding the modules Rikka writes, which this
          configuration imports. `nix flake init -t github:project-rikka/packages#modules`
          run inside it creates them.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        assertions = [
          {
            assertion =
              cfg.configuration.flake == null
              || (
                cfg.configuration.modules != null
                && lib.hasPrefix "${cfg.configuration.flake}/" cfg.configuration.modules
              );
            message = "rikka.configuration.modules (${toString cfg.configuration.modules}) must be inside rikka.configuration.flake (${cfg.configuration.flake}).";
          }
        ];

        environment.systemPackages = [
          (lib.hiPrio pamHelper)
        ]
        ++ installed
        ++ (with pkgs; [
          adwaita-icon-theme
          git
          hwdata
          xdg-user-dirs
          xdg-utils
          xkeyboard-config
        ]);

        environment.pathsToLink = [
          "/share/hwdata"
          "/share/keymaps"
          "/share/thumbnailers"
          "/share/xdg-terminal-exec"
        ];

        environment.etc."rikka/catalog.json".text = builtins.toJSON {
          components = lib.mapAttrs (id: entry: {
            inherit (entry)
              name
              summary
              offer
              version
              ;
            installed = lib.elem id chosen;
          }) components;
        };

        environment.etc."rikka/configuration.json" = lib.mkIf (cfg.configuration.flake != null) {
          text = builtins.toJSON {
            inherit (cfg.configuration) flake modules;
          };
        };

        systemd.packages = installed ++ [ pkgs.xdg-user-dirs ];

        systemd.user.services.xdg-user-dirs.wantedBy = [ "graphical-session-pre.target" ];

        xdg.portal = {
          enable = true;
          extraPortals = lib.filter (package: package.passthru.portal) installed ++ [
            pkgs.xdg-desktop-portal-gtk
          ];
          configPackages = [ base ];
        };

        security.wrappers.rikka-compositor = {
          source = lib.getExe' base "rikka-compositor";
          owner = "root";
          group = "root";
          capabilities = "cap_sys_nice+ep";
        };

        security.pam.services = lib.genAttrs (lib.concatMap (id: components.${id}.pam) chosen) (_: { });

        security.polkit.enable = true;
        security.rtkit.enable = lib.mkDefault true;

        services.displayManager.sessionPackages = [ base ];

        hardware.graphics.enable = lib.mkDefault true;
        programs.xwayland.enable = lib.mkDefault true;
        programs.dconf.enable = lib.mkDefault true;
        xdg.icons.fallbackCursorThemes = lib.mkDefault [ "Adwaita" ];

        services.pipewire = {
          enable = lib.mkDefault true;
          alsa.enable = lib.mkDefault true;
          pulse.enable = lib.mkDefault true;
        };
        hardware.bluetooth.enable = lib.mkDefault true;
        services.udisks2.enable = lib.mkDefault true;
        services.accounts-daemon.enable = lib.mkDefault true;
        services.upower.enable = lib.mkDefault true;
        services.power-profiles-daemon.enable = lib.mkDefault (
          !config.services.tlp.enable && !config.services.auto-cpufreq.enable
        );
        services.geoclue2 = {
          enable = lib.mkDefault true;
          appConfig.rikka-shell = {
            isAllowed = true;
            isSystem = true;
          };
        };
        networking.networkmanager.plugins = lib.mkIf config.networking.networkmanager.enable [
          pkgs.networkmanager-openvpn
        ];

        fonts.packages = with pkgs; [
          inter
          jetbrains-mono
          noto-fonts
          noto-fonts-cjk-sans
          noto-fonts-color-emoji
        ];
        fonts.fontconfig.defaultFonts = {
          sansSerif = lib.mkDefault [ "Inter" ];
          monospace = lib.mkDefault [ "JetBrains Mono" ];
        };
      }

      {
        environment.etc = {
          "rikka/web/chrome".source = lib.getExe' chrome "google-chrome-stable";
          "rikka/web/host".source = lib.getExe' base "rikka-web-host";
          "rikka/web/policies/managed/rikka.json".text = builtins.toJSON {
            ExtensionSettings = lib.listToAttrs (
              map (
                extension:
                lib.nameValuePair extension.id {
                  installation_mode = "force_installed";
                  update_url = "file:///etc/${updateManifest extension}";
                  override_update_url = true;
                }
              ) extensions
            );
            DefaultBrowserSettingEnabled = false;
            TranslateEnabled = false;
          };
        }
        // lib.listToAttrs (
          map (
            extension: lib.nameValuePair (updateManifest extension) { source = updateResponse extension; }
          ) extensions
        );

        systemd.tmpfiles.rules = [ "d /etc/opt/chrome/policies 0755 root root - -" ];
      }

      (lib.mkIf (cfg.autologin.user != null) {
        assertions = [
          {
            assertion = config.users.mutableUsers || lib.hasAttr cfg.autologin.user config.users.users;
            message = "rikka.autologin.user names ${cfg.autologin.user}, who is not in users.users, and users.mutableUsers is off.";
          }
          {
            assertion = otherDisplayManagers == [ ];
            message = "rikka.autologin.user starts Rikka on tty1 in place of a display manager, but systemd.services.${lib.head otherDisplayManagers} runs another one. Turn that display manager off, or unset rikka.autologin.user and choose the Rikka session in it.";
          }
        ];

        services.displayManager.enable = true;
        services.xserver.displayManager.lightdm.enable = overNixosDefault false;

        security.pam.services.${autologin}.startSession = true;

        systemd.services.${autologin} = {
          description = "Rikka on tty1";
          aliases = [ "display-manager.service" ];
          wantedBy = [ "graphical.target" ];
          wants = [
            "dbus.socket"
            "systemd-user-sessions.service"
          ];
          after = [
            "systemd-user-sessions.service"
            "systemd-logind.service"
            console
          ];
          conflicts = [ console ];
          onSuccess = [ console ];
          onFailure = [ console ];
          restartIfChanged = false;
          unitConfig.ConditionPathExists = "/dev/tty1";
          environment = {
            XDG_SESSION_TYPE = "wayland";
            XDG_SESSION_CLASS = "user";
            XDG_SESSION_DESKTOP = "rikka";
          };
          script = "exec \"$SHELL\" -l -c 'exec rikka-compositor'";
          serviceConfig = {
            User = cfg.autologin.user;
            PAMName = autologin;
            TTYPath = "/dev/tty1";
            TTYReset = true;
            TTYVHangup = true;
            TTYVTDisallocate = true;
            StandardInput = "tty-fail";
            StandardOutput = "journal";
            StandardError = "journal";
            UtmpIdentifier = "tty1";
            UtmpMode = "user";
            IgnoreSIGPIPE = false;
          };
        };
      })
    ]
  );
}
