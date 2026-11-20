#!{bash}
set -eu

binary="$PWD/{binary}"
runner={runner}
seconds={limit}
jobs={jobs}
arguments=("$@")
session_runtime="${XDG_RUNTIME_DIR:-}"

export LANG=C.UTF-8 TZ=UTC
export PATH="{path}"
export FONTCONFIG_FILE="{fontconfig_file}"
export XDG_DATA_DIRS="{xdg_data_dirs}"
export SSL_CERT_FILE="{ssl_cert_file}"
unset WAYLAND_DISPLAY WAYLAND_SOCKET DISPLAY DBUS_SESSION_BUS_ADDRESS

scratch="$(mktemp -d /tmp/rikka-test.XXXXXX)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/driver/mesa" "$scratch/driver/nvidia"
export MESA_SHADER_CACHE_DIR="$scratch/driver/mesa"
export __GL_SHADER_DISK_CACHE_PATH="$scratch/driver/nvidia"

isolate() {
    local root="$scratch/$1"
    for directory in home config data state cache runtime tmp system; do
        mkdir -p "$root/$directory"
    done
    chmod 700 "$root/runtime"
    export HOME="$root/home"
    export XDG_CONFIG_HOME="$root/config"
    export XDG_DATA_HOME="$root/data"
    export XDG_STATE_HOME="$root/state"
    export XDG_CACHE_HOME="$root/cache"
    export XDG_CONFIG_DIRS="$root/system"
    export TMPDIR="$root/tmp" TMP="$root/tmp" TEMP="$root/tmp" TEMPDIR="$root/tmp"
    export DBUS_SYSTEM_BUS_ADDRESS="unix:path=$root/runtime/no-system-bus"
    if [ -n "$session_runtime" ]; then
        export XDG_RUNTIME_DIR="$session_runtime"
        export PIPEWIRE_RUNTIME_DIR="$session_runtime"
        export PULSE_RUNTIME_PATH="$session_runtime/pulse"
    else
        export XDG_RUNTIME_DIR="$root/runtime"
    fi
}

if [ -n "${TEST_SHARD_STATUS_FILE:-}" ]; then
    touch "$TEST_SHARD_STATUS_FILE"
fi

if [ "$runner" = program ]; then
    isolate program
    "$binary" "${arguments[@]}"
    exit
fi

names=()
logs=()
failed=()

start() {
    local index=$1 name=$2
    (
        isolate "$index"
        exec timeout --kill-after=10 "$seconds" "$binary" "${arguments[@]}" --exact "$name" --test-threads=1 \
            </dev/null >"$scratch/$index.log" 2>&1
    ) &
    names[$!]=$name
    logs[$!]="$scratch/$index.log"
}

collect() {
    local pid status=0
    wait -n -p pid || status=$?
    cat "${logs[$pid]}"
    if [ "$status" -eq 124 ]; then
        echo "test ${names[$pid]} timed out after $seconds seconds" >&2
    fi
    if [ "$status" -ne 0 ]; then
        failed+=("${names[$pid]}")
    fi
    unset "names[$pid]" "logs[$pid]"
}

isolate list
listed="$("$binary" "${arguments[@]}" --list --format=terse)"
shards="${TEST_TOTAL_SHARDS:-1}"
shard="${TEST_SHARD_INDEX:-0}"
index=0
while IFS= read -r line; do
    case $line in
        *": test") name=${line%: test} ;;
        *) continue ;;
    esac
    if [ $((index % shards)) -eq "$shard" ]; then
        if [ "${#names[@]}" -ge "$jobs" ]; then
            collect
        fi
        start "$index" "$name"
    fi
    index=$((index + 1))
done <<<"$listed"
while [ "${#names[@]}" -gt 0 ]; do
    collect
done

if [ "${#failed[@]}" -gt 0 ]; then
    printf 'failed: %s\n' "${failed[@]}" >&2
    exit 1
fi
