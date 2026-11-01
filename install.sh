#!/bin/sh
# Installs or updates Horizon Studio in Horizon's launcher, in seconds where it can.
#
#   curl -fsSL https://raw.githubusercontent.com/vermingov/horizon-studio/main/install.sh | sh
#
# On Windows, install.ps1 does the same in PowerShell (its key computation is this key_of's twin):
#   irm https://raw.githubusercontent.com/vermingov/horizon-studio/main/install.ps1 | iex
#
# Horizon Studio is a native mod: the launcher compiles it into the game (10 to 30 minutes).
# Each release also carries that game already built, for Linux (x86_64) and macOS (Apple
# silicon), built the way the launcher builds it. This script puts it in the launcher's
# versions folder under the name the launcher gives that build, then runs the launcher's own
# `native-build`, which finds it there and only selects it for an instance. The bundle goes to
# the launcher as its registry ships mods (horizon-native.json beside mixins/<id>/), pinning the
# horizon-mixins commit the game was built with: launchers name a build after the pinned commit,
# where a bare bundle would get the launcher's own, which changes with its updates.
# Where there is no prebuilt game (another platform), the launcher compiles as before, with
# settings that compile faster.
#
# Close the launcher first. Environment:
#   HORIZON_LAUNCHER       the launcher program (default: horizon-launcher, or the macOS app's)
#   HORIZON_LAUNCHER_HOME  the launcher's data folder, as the launcher reads it
#   HORIZON_STUDIO_BUILD=1 compile instead of using the prebuilt game
set -eu

REPO=vermingov/horizon-studio
ID=horizon_studio

say() { printf '%s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -c1-64
    else shasum -a 256 | cut -c1-64; fi
}

# n as 8 little-endian bytes.
le64() {
    n=$1 i=0
    while [ "$i" -lt 8 ]; do
        printf "\\$(printf '%03o' $((n % 256)))"
        n=$((n / 256)) i=$((i + 1))
    done
}

# The launcher's version_for (launcher/src/native_mods.rs) for one flat bundle folder:
# native-<7 digits of the revision>-<12 hex digits of SHA-256 over the inputs, each followed
# by a NUL, then the mod's id and a NUL, then each file (by name) as name, length, bytes>.
key_of() { # bundle mixins profile features rust revision
    digest=$(
        {
            printf '%s\000' "$2" "$3" "$4" "$5" "$6" "$ID"
            for f in $(cd "$1" && LC_ALL=C ls); do
                printf '%s' "$f"
                le64 "$(wc -c < "$1/$f" | tr -d ' ')"
                cat "$1/$f"
            done
        } | sha256
    )
    printf 'native-%s-%s\n' "$(printf '%s' "$6" | cut -c1-7)" "$(printf '%s' "$digest" | cut -c1-12)"
}

# For tests: the key of a bundle, to compare with tools/studio.py native-key.
if [ "${1:-}" = "--key" ]; then
    shift
    key_of "$@"
    exit 0
fi

# ----- The launcher, its data folder, the platform -----

launcher=${HORIZON_LAUNCHER:-}
if [ -z "$launcher" ]; then
    if command -v horizon-launcher >/dev/null 2>&1; then
        launcher=horizon-launcher
    elif [ -x "/Applications/Horizon.app/Contents/MacOS/horizon-launcher" ]; then
        launcher="/Applications/Horizon.app/Contents/MacOS/horizon-launcher"
    else
        die "horizon-launcher not found: install Horizon's launcher, or set HORIZON_LAUNCHER to it."
    fi
fi

case "$(uname -s)" in
    Linux) data=${HORIZON_LAUNCHER_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon-launcher} ;;
    Darwin) data=${HORIZON_LAUNCHER_HOME:-$HOME/Library/Application Support/horizon-launcher} ;;
    *) data=${HORIZON_LAUNCHER_HOME:-} ;;
esac
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
    Darwin-arm64) target=aarch64-apple-darwin ;;
    *) target= ;;
esac

cache=${XDG_CACHE_HOME:-$HOME/.cache}/horizon-studio
mkdir -p "$cache"

# The installed Horizon version to build on: the newest prepared one that isn't itself a native
# build, preferring one of the game version the studio is made for.
base_version() { # game
    versions="$data/versions"
    [ -d "$versions" ] || return 0
    best= any=
    for d in $(cd "$versions" && ls -1 | LC_ALL=C sort -r); do
        case "$d" in native-*|.*) continue ;; esac
        [ -f "$versions/$d/.installed" ] || continue
        ls "$versions/$d"/.local/horizon-client/*/horizon-resources.json >/dev/null 2>&1 || continue
        [ -n "$any" ] || any=$d
        if [ -z "$best" ] && grep -q "\"version\": *\"$1\"" "$versions/$d/research-lock.json" 2>/dev/null; then
            best=$d
        fi
    done
    printf '%s\n' "${best:-$any}"
}

# Copies a folder's contents, cloned where the disk can.
copy_tree() { # from to
    for entry in "$1"/* "$1"/.[!.]*; do
        [ -e "$entry" ] || continue
        case "${entry##*/}" in horizon-client|.installed|Horizon.app|native-mods.json) continue ;; esac
        cp -Rc "$entry" "$2/" 2>/dev/null || cp -a --reflink=auto "$entry" "$2/" 2>/dev/null \
            || cp -R "$entry" "$2/" || return 1
    done
}

# ----- The prebuilt game -----
# Called as `prebuilt || compile`, where sh ignores `set -e`: each step checks itself. A
# problem getting the prebuilt game falls back to compiling; one installing it stops.

prebuilt() {
    [ -n "$target" ] && [ -n "$data" ] || return 1
    [ "${HORIZON_STUDIO_BUILD:-}" != 1 ] || return 1
    dir="$cache/prebuilt"
    rm -rf "$dir" && mkdir -p "$dir" || return 1
    asset="horizon-studio-game-$target.tar.gz"
    url=${HORIZON_STUDIO_RELEASE_URL:-https://github.com/$REPO/releases/latest/download}
    # Looked for quietly first: a release from before prebuilt games, or a build that failed, has none.
    curl -fsIL -o /dev/null "$url/$asset" 2>/dev/null \
        || { say "The latest release has no prebuilt game for $target."; return 1; }
    say "Downloading Horizon Studio, built for $target..."
    curl -fL --progress-bar -o "$dir/$asset" "$url/$asset" || { say "Could not download $asset."; return 1; }
    curl -fsSL -o "$dir/SHA256SUMS" "$url/SHA256SUMS" || { say "The release has no checksums."; return 1; }
    want=$(awk -v f="$asset" '$2 == f { print $1 }' "$dir/SHA256SUMS")
    got=$(sha256 < "$dir/$asset")
    [ -n "$want" ] && [ "$want" = "$got" ] || die "$asset does not match the release's checksum."
    tar -xzf "$dir/$asset" -C "$dir" && rm -f "$dir/$asset" || { say "Could not unpack $asset."; return 1; }
    for need in horizon-client key-inputs LICENSE-Horizon "bundle/$ID/horizon-mixin.toml"; do
        [ -f "$dir/$need" ] || { say "The prebuilt game has no $need."; return 1; }
    done
    # key-inputs: the horizon-mixins commit, profile, features, Rust, Horizon commit and game it was built with.
    { read -r mixins; read -r profile; read -r features; read -r rust; read -r revision; read -r game; } < "$dir/key-inputs"
    # The bundle as the launcher's registry ships a mod, with the commits the game was built with.
    native="$dir/native"
    bundle="$native/mixins/$ID"
    mkdir -p "$native/mixins" && mv "$dir/bundle/$ID" "$bundle" || return 1
    modversion=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$bundle/horizon-mixin.toml" | head -1)
    printf '{"schema": 1, "id": "%s", "version": "%s", "game": "%s", "horizon": "%s", "mixins": "%s"}\n' \
        "$ID" "$modversion" "$game" "$revision" "$mixins" > "$native/horizon-native.json" || return 1
    # The key names the launcher's Rust: this launcher's, which may be newer than the one it was built with.
    theirs=$("$launcher" --json rust status 2>/dev/null | sed -n 's/.*"pinned": *"\([^"]*\)".*/\1/p' | head -1)
    version=$(key_of "$bundle" "$mixins" "$profile" "$features" "${theirs:-$rust}" "$revision") || return 1
    # Launchers that build with a quicker profile by default say which one goes in the name.
    choose=
    if "$launcher" native-build --help 2>/dev/null | grep -q -- '--profile'; then
        choose="--profile $profile"
    fi
    base=$(base_version "$game")
    [ -n "$base" ] || die "no prepared Horizon in the launcher yet: open it, install Horizon and play it once (or \`$launcher prepare\`), then run this again."
    seeded="$data/versions/$version"
    if [ ! -f "$seeded/.installed" ]; then
        say "Installing it as $version (on $base)..."
        partial="$data/versions/.$version.partial"
        rm -rf "$partial" && mkdir -p "$partial" || die "could not make $partial."
        { copy_tree "$data/versions/$base" "$partial" \
            && cp "$dir/horizon-client" "$partial/horizon-client" \
            && chmod 755 "$partial/horizon-client" \
            && cp "$dir/LICENSE-Horizon" "$partial/" \
            && printf '{"base": "%s", "horizon": "%s", "horizon_mixins": "%s", "rust": "%s", "profile": "%s", "mods": [{"id": "%s", "version": "%s"}], "prebuilt": "%s"}\n' \
                "$base" "$revision" "$mixins" "$rust" "$profile" "$ID" "$modversion" "$target" > "$partial/native-mods.json"; } \
            || { rm -rf "$partial"; die "could not install the game in $data/versions."; }
        # Marked installed last, once whole: the launcher only lists folders with .installed.
        rm -rf "$seeded" && mv "$partial" "$seeded" && : > "$seeded/.installed" \
            || die "could not install the game as $seeded."
    fi
    # The launcher's own step: it finds the build in place and selects it for an instance (the
    # selected one; older launchers make a Native mods instance, with your mod settings). A
    # launcher that names the build otherwise compiles instead.
    # shellcheck disable=SC2086 # $choose is two words or none
    out=$("$launcher" native-build "$native" --version "$base" $choose --trust-native-code) || exit 1
    say "$out"
    case "$out" in
        "$version "*) say "Done: open the launcher and press Play (older launchers: on the Native mods instance)." ;;
        *) say "Your launcher compiled it instead (it names this build differently); open the launcher and press Play." ;;
    esac
    return 0
}

# ----- Compiling, as before -----

compile() {
    say "Compiling Horizon Studio into the game (the first time takes a while; later ones are faster)..."
    src="$cache/horizon-studio-main"
    rm -rf "$src"
    curl -fsSL "https://github.com/$REPO/archive/refs/heads/main.tar.gz" | tar -xz -C "$cache" \
        || die "could not download Horizon Studio's source."
    # Optimised as the release profile is, without its slow whole-program steps: the game
    # compiles and links several times faster, and a rebuild only redoes what changed.
    CARGO_PROFILE_RELEASE_LTO=${CARGO_PROFILE_RELEASE_LTO:-false} \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-16} \
    CARGO_PROFILE_RELEASE_INCREMENTAL=${CARGO_PROFILE_RELEASE_INCREMENTAL:-true} \
        "$launcher" native-build "$src/mixins/$ID" --trust-native-code || exit 1
    say "Done: open the launcher and press Play (older launchers: on the Native mods instance)."
}

prebuilt || compile
