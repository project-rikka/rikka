{ release, packages }:

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

  outdated = lib.filter (
    id:
    components.${id}.requires != null
    && lib.versionOlder components.base.version components.${id}.requires
  ) chosen;

  autologin = "rikka-autologin";

  console = "getty@tty1.service";
in
{
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
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        assertions = map (id: {
          assertion = false;
          message = "Rikka ${id} ${components.${id}.version} needs the Rikka desktop ${components.${id}.requires} or newer, but this distribution has ${components.base.version}.";
        }) outdated;

        environment.systemPackages = [
          (lib.hiPrio pamHelper)
        ]
        ++ installed
        ++ (with pkgs; [
          adwaita-icon-theme
          hwdata
          xdg-user-dirs
          xdg-utils
          xkeyboard-config
        ])
        ++ lib.optional (cfg.apps.ade.enable or false) pkgs.git;

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
        services.power-profiles-daemon.enable = lib.mkDefault true;
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

      (lib.mkIf (cfg.autologin.user != null) {
        services.displayManager.enable = true;

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
