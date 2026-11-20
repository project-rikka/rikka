#!/bin/sh
#
# Fetch the Rikka NixOS installer and run it.
#
#   curl -fsSL https://install.rikka.sh | sudo sh
#
# Uses a released binary when there is one and builds from source with the live
# medium's nix when there is not. If neither works it says which and stops.
#
# POSIX sh: the live medium has bash, but this is the first thing that runs on
# somebody else's machine, so it aims at the narrowest shell there is.
#
set -eu

REPO=${RIKKA_REPO:-project-rikka/rikka}
REF=${RIKKA_REF:-main}
RELEASES=${RIKKA_RELEASES:-https://github.com/project-rikka/packages/releases/download/binaries}
SYSTEM=x86_64-linux

die() {
    printf '\033[31merror:\033[0m %s\n' "$1" >&2
    exit 1
}

info() {
    printf '\033[36m==>\033[0m %s\n' "$1" >&2
}

have() {
    command -v "$1" >/dev/null 2>&1
}

# curl or wget. A live medium has one of them.
fetch() {
    url=$1
    out=$2
    if have curl; then
        curl -fsSL --retry 3 -o "$out" "$url"
    elif have wget; then
        wget -q -O "$out" "$url"
    else
        die 'neither curl nor wget is here.'
    fi
}

require_x86_64() {
    [ "$(uname -m)" = x86_64 ] || die "unsupported CPU: $(uname -m). Rikka runs on x86_64 only."
}

# The released binary. Not used unless its SHA-256 matches.
download_release() {
    into=$1
    name="rikka-installer-$SYSTEM"

    info "looking for a release: $RELEASES/$name"
    fetch "$RELEASES/$name" "$into" 2>/dev/null || return 1
    fetch "$RELEASES/$name.sha256" "$into.sha256" 2>/dev/null || return 1

    # `return`, not `die`: the caller reads a non-zero return as "build from source",
    # and an unverifiable download is exactly a reason to do that.
    have sha256sum || return 1
    expected=$(cut -d' ' -f1 <"$into.sha256")
    actual=$(sha256sum "$into" | cut -d' ' -f1)
    [ "$expected" = "$actual" ] || die "SHA-256 mismatch: expected $expected, got $actual"

    chmod +x "$into"
    info 'release verified.'
}

# Build it. The live medium has nix, so the whole toolchain is borrowed from the
# source's own development shell.
build_from_source() {
    into=$1
    work=$2

    have nix || die 'no release found and no nix to build with.'

    info "fetching the source: $REPO@$REF"
    fetch "https://github.com/$REPO/archive/$REF.tar.gz" "$work/source.tar.gz" ||
        die 'could not fetch the source. Check the network (nmtui).'
    tar -xzf "$work/source.tar.gz" -C "$work"

    root=$(find "$work" -maxdepth 1 -type d -name "${REPO##*/}-*" | head -n 1)
    [ -n "$root" ] || die 'the unpacked source is not shaped as expected.'

    info 'bazel build (several minutes)'
    flags='--config=local --config=installer'
    build="bazel build $flags //installer:rikka-installer &&
        echo \"\$(bazel info $flags execution_root)/\$(bazel cquery $flags --output=files //installer:rikka-installer)\" >'$work/built'"

    (cd "$root" &&
        nix --extra-experimental-features 'nix-command flakes' \
            develop --command sh -c "$build") ||
        die 'the build failed.'

    built=$(cat "$work/built")
    [ -x "$built" ] || die 'it built, but the result is nowhere to be found.'
    cp "$built" "$into"
    chmod +x "$into"
}

main() {
    require_x86_64

    # The trap catches an early exit. It deliberately does not fire on the way out:
    # `exec` replaces this shell, and the binary it hands over to is inside $work.
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT INT TERM

    binary="$work/rikka-installer"
    download_release "$binary" || {
        info 'no release, so building from source.'
        build_from_source "$binary" "$work"
    }

    # Run under this terminal. When this script is being piped, standard input is the
    # pipe, so the keyboard is taken from /dev/tty instead.
    if [ -t 0 ]; then
        exec "$binary" "$@"
    elif [ -c /dev/tty ]; then
        exec "$binary" "$@" </dev/tty
    else
        die 'no terminal. Run this from a tty.'
    fi
}

main "$@"
