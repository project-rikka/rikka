#!/usr/bin/env bash
#
# Boot a NixOS live medium in qemu and hand it the installer that was just built.
#
# --demo is for looking at the screens; this is for proving the thing: UEFI
# firmware, one empty disk, and sgdisk, cryptsetup and nixos-install all real.
#
set -euo pipefail

readonly HERE="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT="$(dirname -- "$HERE")"
readonly WORK="${RIKKA_VM_DIR:-$HERE/vm}"
readonly CHANNEL="${RIKKA_CHANNEL:-nixos-26.05}"
readonly DISK_SIZE="${RIKKA_DISK:-64G}"
readonly MEMORY="${RIKKA_MEMORY:-4096}"

usage() {
    cat <<'EOF'
usage: ./test/qemu.sh [options]

Boots the NixOS minimal ISO under UEFI qemu and shares installer/dist over 9p.
Inside the guest:

  mkdir -p /mnt/host
  mount -t 9p -o trans=virtio,version=9p2000.L host /mnt/host
  /mnt/host/rikka-installer-x86_64-linux

options:
  --fresh     recreate the disk image, throwing away the last run
  --boot      boot the installed disk instead of the ISO
  -h, --help

environment:
  RIKKA_ISO      an ISO to use (otherwise one is fetched from channels.nixos.org)
  RIKKA_CHANNEL  which channel to fetch (default: nixos-26.05)
  RIKKA_VM_DIR   working directory (default: installer/test/vm)
  RIKKA_DISK     disk size (default: 64G)
  RIKKA_MEMORY   memory in MiB (default: 4096)
EOF
}

die() {
    printf '\033[31merror:\033[0m %s\n' "$1" >&2
    exit 1
}

# To stderr: `fetch_iso`'s stdout is the path its caller captures.
info() {
    printf '\033[36m==>\033[0m %s\n' "$1" >&2
}

# OVMF lives in a different place on every distribution. Two files are needed: the
# read-only code and a writable NVRAM.
#
# On NixOS none of the FHS paths exist, but qemu itself ships the edk2 images in its own
# `share/qemu`, so the first candidate is built from wherever the binary about to be run
# came from -- which is the store path under `nix-shell -p qemu` as much as it is the
# system profile.
find_firmware() {
    local prefix
    prefix="$(dirname -- "$(dirname -- "$(readlink -f -- "$(command -v qemu-system-x86_64)")")")"
    local -a candidates=(
        "$prefix/share/qemu/edk2-x86_64-code.fd:$prefix/share/qemu/edk2-i386-vars.fd"
        /run/current-system/sw/FV/OVMF_CODE.fd:/run/current-system/sw/FV/OVMF_VARS.fd
        /usr/share/edk2/x64/OVMF_CODE.4m.fd:/usr/share/edk2/x64/OVMF_VARS.4m.fd
        /usr/share/edk2/x64/OVMF_CODE.fd:/usr/share/edk2/x64/OVMF_VARS.fd
        /usr/share/OVMF/OVMF_CODE.fd:/usr/share/OVMF/OVMF_VARS.fd
        /usr/share/edk2-ovmf/x64/OVMF_CODE.fd:/usr/share/edk2-ovmf/x64/OVMF_VARS.fd
        /usr/share/qemu/edk2-x86_64-code.fd:/usr/share/qemu/edk2-i386-vars.fd
    )
    local pair
    for pair in "${candidates[@]}"; do
        if [[ -r ${pair%%:*} && -r ${pair##*:} ]]; then
            printf '%s\n' "$pair"
            return 0
        fi
    done
    die 'no OVMF found. On NixOS: nix-shell -p qemu, or put OVMF.fd in environment.systemPackages'
}

fetch_iso() {
    local iso="$WORK/nixos-minimal.iso"
    if [[ -n ${RIKKA_ISO-} ]]; then
        printf '%s\n' "$RIKKA_ISO"
        return 0
    fi
    if [[ ! -f $iso ]]; then
        info "fetching the ISO: $CHANNEL"
        curl -fL --progress-bar \
            -o "$iso.part" \
            "https://channels.nixos.org/$CHANNEL/latest-nixos-minimal-x86_64-linux.iso"
        mv "$iso.part" "$iso"
    fi
    printf '%s\n' "$iso"
}

main() {
    local fresh=0 boot=0
    while (($# > 0)); do
        case $1 in
        --fresh) fresh=1 ;;
        --boot) boot=1 ;;
        -h | --help)
            usage
            return 0
            ;;
        *)
            usage >&2
            die "unknown option: $1"
            ;;
        esac
        shift
    done

    command -v qemu-system-x86_64 >/dev/null || die 'no qemu-system-x86_64.'
    mkdir -p "$WORK"

    local asset="$ROOT/dist/rikka-installer-x86_64-linux"
    if [[ ! -x $asset ]]; then
        info 'not built yet, building'
        command -v bazel >/dev/null || die 'no bazel -- `nix develop` is where it comes from.'
        local built
        built=$(cd "$ROOT/.." &&
            bazel build --config=installer --remote_download_toplevel //installer:rikka-installer &&
            echo "$(bazel info --config=installer execution_root)/$(bazel cquery --config=installer --output=files //installer:rikka-installer)") ||
            die 'the build failed.'
        mkdir -p "$(dirname -- "$asset")"
        install -m 755 "$built" "$asset"
    fi

    local disk="$WORK/target.qcow2"
    if ((fresh)) || [[ ! -f $disk ]]; then
        rm -f "$disk"
        qemu-img create -f qcow2 "$disk" "$DISK_SIZE" >/dev/null
        info "made an empty disk ($DISK_SIZE)"
    fi

    local firmware code vars
    firmware="$(find_firmware)"
    code="${firmware%%:*}"
    vars="$WORK/OVMF_VARS.fd"
    if ((fresh)) || [[ ! -f $vars ]]; then
        cp "${firmware##*:}" "$vars"
        chmod u+w "$vars"
    fi

    # `-cpu host` is only meaningful under KVM; with TCG it fails outright, so the
    # fallback has to pick a model qemu can emulate.
    local cpu=max
    [[ -r /dev/kvm ]] && cpu=host

    local -a args=(
        -machine q35,accel=kvm:tcg
        -cpu "$cpu"
        -smp "$(nproc)"
        -m "$MEMORY"
        -drive "if=pflash,format=raw,readonly=on,file=$code"
        -drive "if=pflash,format=raw,file=$vars"
        -drive "if=virtio,format=qcow2,file=$disk"
        -netdev user,id=net0
        -device virtio-net-pci,netdev=net0
        # The built binary, visible to the guest at
        # `mount -t 9p -o trans=virtio,version=9p2000.L host /mnt/host`.
        -virtfs "local,path=$ROOT/dist,mount_tag=host,security_model=none,id=host"
        -display gtk
        -vga virtio
    )

    if ((boot)); then
        info 'booting the installed disk'
    else
        local iso
        iso="$(fetch_iso)"
        args+=(-drive "media=cdrom,format=raw,readonly=on,file=$iso" -boot order=d)
        info "booting from $iso"
        cat <<EOF

Once the guest is up:

  sudo -i
  mkdir -p /mnt/host && mount -t 9p -o trans=virtio,version=9p2000.L host /mnt/host
  /mnt/host/rikka-installer-x86_64-linux

Then poweroff and run \`./test/qemu.sh --boot\` to start what was installed.

EOF
    fi

    exec qemu-system-x86_64 "${args[@]}"
}

main "$@"
