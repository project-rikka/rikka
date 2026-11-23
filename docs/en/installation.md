# Installing Rikka

Rikka is a desktop for NixOS on x86_64 machines. It ships as prebuilt packages and
a NixOS module in the flake `github:project-rikka/packages`, so you install it by
building it into your NixOS configuration.

You can install it in one of two ways:

- [Install NixOS with Rikka from scratch](#install-nixos-with-rikka-from-scratch).
  The installer partitions your disk and sets everything up for you.
- [Add Rikka to an existing NixOS system](#add-rikka-to-an-existing-nixos-system).
  You keep your current configuration and add Rikka to it.

## Requirements

- An x86_64 machine.
- A GPU with Vulkan 1.3 and a kernel modesetting (KMS) driver: Intel, AMD, or
  NVIDIA Turing (GTX 16xx / RTX 20xx) or newer. Most virtual machines cannot run
  the desktop because their virtual GPUs lack the features the compositor requires.
- NixOS unstable (recommended) or the latest stable release.
- A flake-based configuration. If you still use channels, see step 1 below to
  convert yours.

## Install NixOS with Rikka from scratch

1. Boot the machine from a [NixOS live USB](https://nixos.org/download/#nixos-iso)
   in UEFI mode and connect to the network (`nmtui` for Wi-Fi).
2. Run:

   ```sh
   curl -fsSL https://install.rikka.sh | sudo sh
   ```

3. Follow the prompts. The installer erases the disk you choose, generates
   `/etc/nixos`, and installs NixOS with Rikka. Reboot when it's done.

Because it erases a disk, the script aborts if the system was booted from an
internal disk rather than a live environment. If you already have NixOS installed,
follow the next section instead.

## Add Rikka to an existing NixOS system

Rikka requires the following in your configuration:

1. The flake input `rikka`.
2. The NixOS module `rikka.nixosModules.default`.
3. A `rikka/` directory imported by your configuration. Rikka's Settings and
   Packages apps write your choices into this directory.
4. `rikka.enable = true;`

These steps assume your flake is in `/etc/nixos`. If it's elsewhere, see
[Configurations outside /etc/nixos](#configurations-outside-etcnixos).

### 1. Use a flake

Skip this step if `/etc/nixos/flake.nix` already exists.

On a system without flakes, add `--extra-experimental-features 'nix-command flakes'`
to every `nix` command until the rebuild in step 5 enables flakes. Run:

```sh
cd /etc/nixos
sudo nix --extra-experimental-features 'nix-command flakes' \
  flake init -t github:project-rikka/packages
```

This creates `flake.nix` and `rikka/` next to your `configuration.nix`. It also
covers steps 2 and 3, so you can skip ahead to step 4.

The generated `flake.nix` builds `./configuration.nix` with Rikka and names the
system after `networking.hostName`. It expects `hardware-configuration.nix` to
set `nixpkgs.hostPlatform` (which `nixos-generate-config` adds automatically).
If yours lacks it, add `nixpkgs.hostPlatform = "x86_64-linux";`. Flakes cannot
read channels from `NIX_PATH`, so replace imports such as `<nixpkgs/…>` or
`<home-manager/…>` with flake inputs.

The generated flake follows the nixpkgs revision Rikka was built against, which
is NixOS unstable. To stay on a stable release, see [Choosing nixpkgs](#choosing-nixpkgs).

### 2. Add the Rikka input

In `flake.nix`, add the input:

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  rikka.url = "github:project-rikka/packages";
};
```

Do not set `rikka.inputs.nixpkgs.follows`. See
[Choosing nixpkgs](#choosing-nixpkgs).

### 3. Add the Rikka module and directory

Initialize the `rikka/` directory with Rikka's template modules:

```sh
sudo mkdir /etc/nixos/rikka
cd /etc/nixos/rikka
sudo nix flake init -t github:project-rikka/packages#modules
```

Then add the module and the directory to your system's `modules`:

```nix
outputs = { nixpkgs, rikka, ... }: {
  nixosConfigurations.laptop = nixpkgs.lib.nixosSystem {
    modules = [
      ./configuration.nix
      rikka.nixosModules.default
      ./rikka
    ];
  };
};
```

You can also put `./rikka` in `configuration.nix`'s `imports`.

### 4. Enable Rikka and configure login

In `configuration.nix`:

```nix
rikka.enable = true;
```

Then choose how Rikka starts:

- **With a display manager.** If you use GDM, SDDM, greetd, or another display
  manager, keep it and select the **Rikka** session on the login screen.
- **Without a display manager.** Set `rikka.autologin.user = "alice";` to start
  Rikka on tty1 as that user at boot without a login screen. Rikka has its own
  lock screen. Disable any other display manager first, since they will conflict.
  NixOS enables LightDM with `services.xserver.enable` when no other display
  manager is set; `rikka.autologin.user` disables that default for you.

### 5. Rebuild

If `/etc/nixos` is a Git repository, stage the new files first. Flakes ignore
untracked files:

```sh
cd /etc/nixos
sudo git add flake.nix flake.lock rikka
```

Then rebuild and reboot:

```sh
sudo nixos-rebuild switch --flake /etc/nixos
```

The first build downloads Rikka's packages and the nixpkgs revision they were
built against, so it will take a while.

## Configurations outside /etc/nixos

The Settings and Packages apps edit the modules in `rikka/` and rebuild your
flake as root. Point Rikka to both:

```nix
rikka.configuration = {
  flake = "/home/alice/nixos";
  modules = "/home/alice/nixos/hosts/laptop/rikka";
};
```

`modules` defaults to `<flake>/rikka` and must live inside the flake. In a flake
with multiple machines, give each machine its own directory, initialize it with
`nix flake init -t github:project-rikka/packages#modules`, and import it from
that machine's configuration. Files written by Rikka retain the directory's
owner, so a configuration in your home directory stays yours.

Rikka rebuilds the configuration matching the machine's hostname, just like
`nixos-rebuild`. If a flake has only a single configuration, it is used regardless
of its name.

To use Rikka without automatic configuration editing, set
`rikka.configuration.flake = null;`. The system pages in Settings and the
Packages app will then show that Rikka does not manage your configuration and
won't modify any files. You do not need a `rikka/` directory in that case.

## Choosing nixpkgs

Rikka's packages are prebuilt and linked against the nixpkgs revision pinned by
`github:project-rikka/packages`. They always use those libraries, regardless of
which nixpkgs your system uses. That includes Mesa: on AMD, Intel and other
Mesa-driven GPUs Rikka draws with its own copy, and only NVIDIA's proprietary
driver comes from your system.

- **NixOS unstable:** set `nixpkgs.follows = "rikka/nixpkgs";`. Your system then
  uses the exact libraries Rikka was built against, so both share one Mesa. Both
  update together, avoiding unnecessary rebuilds.
- **A stable release:** keep your own `nixpkgs.url`, such as
  `github:NixOS/nixpkgs/nixos-26.05`. Rikka keeps its own libraries and Mesa,
  while the rest of your system gets graphics drivers from your release.
- **Never** set `rikka.inputs.nixpkgs.follows = "nixpkgs";`. That relinks Rikka's
  prebuilt binaries against different libraries, which can break them.

## Choosing apps

The module always installs the Rikka desktop and its core apps. Additional apps
are optional. To list them, run:

```sh
nix eval --json github:project-rikka/packages#catalog
```

Every entry whose `offer` is not `required` is optional.

Enable optional apps in one of two ways (avoid using both for the same app):

- In the **Packages** app under **Rikka apps**, which writes your choices to
  `rikka/apps.nix` and rebuilds.
- In your own configuration, for example `rikka.apps.music.enable = true;`.

## Settings Rikka writes for you

The Settings and Packages apps write these options into the modules in `rikka/`.
Remove your own definitions of an option before changing it in Rikka; otherwise,
NixOS will report conflicting definitions since both values have the same priority.

| Where in Rikka | Options |
|---|---|
| Users | `users.users.<you>.description` |
| System | `networking.hostName`, `boot.loader.systemd-boot.configurationLimit`, `nix.gc.*`, `nix.optimise.automatic`, `nix.settings.experimental-features`, `nixpkgs.config.allowUnfree` |
| Date & time | `time.timeZone`, `time.hardwareClockInLocalTime`, `services.automatic-timezoned.enable`, `services.timesyncd.enable`, `networking.timeServers` |
| Language & region, Keyboard | `i18n.defaultLocale`, `i18n.extraLocaleSettings.LC_MESSAGES`, `console.keyMap`, `services.xserver.xkb.layout`, `services.xserver.xkb.variant`, `i18n.inputMethod.fcitx5.addons` |
| Power & battery | `services.logind.settings.Login.*`, `services.upower.*` |
| Remote desktop | `security.pam.services.rikka-remote`, firewall UDP ports |
| Packages | `environment.systemPackages`, `programs.*`, `services.*` you install, `rikka.apps.*` |

Lists such as `environment.systemPackages` merge with your own, so they never
conflict.

## Input methods

Rikka runs fcitx5 itself and displays its candidate window. To type Japanese,
Chinese, Korean, or Vietnamese, enable fcitx5 and disable its autostart service
so it doesn't launch twice:

```nix
i18n.inputMethod = {
  enable = true;
  type = "fcitx5";
  fcitx5.waylandFrontend = true;
  fcitx5.addons = [ pkgs.fcitx5-mozc ];
};
systemd.user.units."app-org.fcitx.Fcitx5@autostart.service".enable = false;
```

You can add more fcitx5 addons directly in Settings under **Keyboard**.

## Graphics drivers

Intel and AMD GPUs work out of the box with NixOS defaults (Rikka enables
`hardware.graphics.enable` automatically).

For NVIDIA, use the open kernel modules with modesetting:

```nix
services.xserver.videoDrivers = [ "nvidia" ];
hardware.nvidia = {
  open = true;
  modesetting.enable = true;
  package = config.boot.kernelPackages.nvidiaPackages.latest;
};
nixpkgs.config.allowUnfree = true;
```

## Things that conflict with Rikka

- **PulseAudio.** Rikka uses PipeWire. Remove `services.pulseaudio.enable = true;`,
  as NixOS refuses to build with both.
- **TLP and auto-cpufreq.** Rikka only enables `power-profiles-daemon` when
  neither is active. Its power profile controls remain unavailable while either
  runs.
- **Another display manager alongside `rikka.autologin.user`.** NixOS will abort
  the build and report the conflict. Use one or the other.

## Updating

The Packages app updates the entire flake—equivalent to running `nix flake update`—and
rebuilds. Progress and logs appear in a notification and in the Updates app.

From a terminal:

```sh
cd /etc/nixos
sudo nix flake update
sudo nixos-rebuild switch --flake /etc/nixos
```

Run `sudo nix flake update rikka` to update only Rikka.

## Removing Rikka

1. Delete `rikka.enable`, `rikka.autologin.user`, and any other `rikka.*` options.
2. Remove `rikka.nixosModules.default` and `./rikka` from your modules.
3. Remove the `rikka` input. If `nixpkgs` follows `rikka/nixpkgs`, point it to a
   nixpkgs URL directly instead.
4. Delete the `rikka/` directory and rebuild.

## Troubleshooting

**Settings says "Rikka does not manage this system's configuration".**
`rikka.configuration.flake` is `null`, or the system was built without the Rikka
module. Set the option and rebuild.

**Settings says the directory "does not hold Rikka's modules".**
The directory in `rikka.configuration.modules` has no `default.nix`. Create the
modules with `nix flake init -t github:project-rikka/packages#modules` inside it.

**The rebuild fails with "path '…/rikka/default.nix' does not exist".**
The files exist, but Git isn't tracking them. Run `git add rikka` in the flake.

**The session starts and falls back to the console.**
Check the logs with `journalctl -b -u rikka-autologin` or, if using a display
manager, `journalctl -b --user`. A missing Vulkan 1.3 driver or a GPU without
KMS is the usual cause.

## Option reference

| Option | Default | Meaning |
|---|---|---|
| `rikka.enable` | `false` | Installs and enables the Rikka desktop. |
| `rikka.apps.<id>.enable` | `false` | Installs the optional app `<id>`. |
| `rikka.autologin.user` | `null` | Starts Rikka on tty1 as this user at boot without a greeter. |
| `rikka.configuration.flake` | `"/etc/nixos"` | The flake Rikka rebuilds. `null` disables automatic configuration editing. |
| `rikka.configuration.modules` | `"<flake>/rikka"` | The directory inside the flake that holds the modules Rikka writes. |