#!/usr/bin/env python3
"""Build Horizon Studio into Horizon, and package it for the launcher.

    python3 tools/studio.py test       # the studio's own tests (no game needed)
    python3 tools/studio.py check      # apply the bundle on the pinned Horizon, cargo check the game
    python3 tools/studio.py build      # ... and build it (fast profile); prints the game's path
    python3 tools/studio.py package    # target/horizon_studio-<version>.hnmod, the launcher's native mod file
    python3 tools/studio.py native-key # the version name the launcher gives this bundle's build
    python3 tools/studio.py prebuilt   # build (release) and pack target/horizon-studio-game-<target>.tar.gz
    python3 tools/studio.py all        # test, check, package

    --horizon PATH   use this Horizon checkout (it needs the pinned commit) instead of GitHub's archive
    --release        build with the release profile (as the launcher does) instead of `fast`

Horizon's source is the commit pinned in compatibility.json; horizon-mixins is built once at its pinned commit.
Everything goes under target/. The bundle is mixins/horizon_studio/; the modules' tests run on their own with
cargo test, and examples.rs is generated from examples/*.lua (tools/gen_examples.py).
"""
import argparse
import hashlib
import io
import re
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tomllib
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "target"
BUNDLE = ROOT / "mixins" / "horizon_studio"
COMPAT = json.loads((ROOT / "compatibility.json").read_text())
MANIFEST = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text())


def log(*args):
    print(*args, file=sys.stderr, flush=True)


def run(cmd, **kw):
    log("$", " ".join(str(c) for c in cmd))
    subprocess.run([str(c) for c in cmd], check=True, **kw)


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "horizon-studio"})
    with urllib.request.urlopen(request, timeout=300) as response:
        return response.read()


def check_pins():
    rev = MANIFEST["compatibility"]["horizon_revision"]
    if rev != COMPAT["horizon_revision"]:
        sys.exit(f"horizon-mixin.toml pins {rev}, compatibility.json {COMPAT['horizon_revision']}")
    if MANIFEST["mod"]["version"] != COMPAT["version"]:
        sys.exit("horizon-mixin.toml and compatibility.json have different versions")


def source(checkout):
    """Horizon's source at the pinned commit, once, in target/horizon/<rev>/ (no links: horizon-mixins refuses them)."""
    rev = COMPAT["horizon_revision"]
    out = TARGET / "horizon" / rev
    if (out / ".horizon-revision").is_file():
        return out
    partial = TARGET / "horizon" / f".{rev}.partial"
    shutil.rmtree(partial, ignore_errors=True)
    partial.mkdir(parents=True)
    if checkout:
        log(f"Exporting Horizon {rev[:7]} from {checkout}")
        data = subprocess.run(["git", "-C", checkout, "archive", "--format=tar", rev], check=True, capture_output=True).stdout
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            archive.extractall(partial / "src", filter="data")
        top = partial / "src"
    else:
        log(f"Downloading Horizon {rev[:7]}")
        data = get(f"https://codeload.github.com/{COMPAT['horizon_repository'].removeprefix('https://github.com/')}/tar.gz/{rev}")
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            archive.extractall(partial, filter="data")
        top = next(partial.iterdir())
    for path in list(top.rglob("*")):
        if path.is_symlink():
            path.unlink()
    (top / ".horizon-revision").write_text(rev)
    shutil.rmtree(out, ignore_errors=True)
    top.rename(out)
    shutil.rmtree(partial, ignore_errors=True)
    return out


def tool():
    """horizon-mixins at the pinned commit, built once into target/horizon-mixins/<rev>/."""
    rev = COMPAT["horizon_mixins_revision"]
    root = TARGET / "horizon-mixins" / rev
    binary = root / "bin" / "horizon-mixins"
    if binary.is_file():
        return binary
    run(["cargo", "install", "--locked", "--git", COMPAT["horizon_mixins_repository"], "--rev", rev,
         "horizon-mixins-cli", "--root", root])
    return binary


def stage(checkout):
    src = source(checkout)
    out = TARGET / "stage"
    run([tool(), "apply", "--workspace", src, "--mods", ROOT / "mixins", "--out", out])
    return out


def cargo_env():
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(TARGET / "game"))
    return env


def cmd_test(_):
    run(["python3", ROOT / "tools" / "gen_examples.py"])
    run(["cargo", "test", "--locked"], cwd=ROOT)


def cmd_check(args):
    check_pins()
    out = stage(args.horizon)
    run(["cargo", "check", "--locked", "-p", "horizon-client", "--bin", "horizon-client"], cwd=out, env=cargo_env())
    log("The modded game type-checks.")


def cmd_build(args):
    check_pins()
    out = stage(args.horizon)
    profile = "release" if args.release else "fast"
    run(["cargo", "build", "--locked", "--profile", profile, "-p", "horizon-client", "--bin", "horizon-client"],
        cwd=out, env=cargo_env())
    exe = Path(cargo_env()["CARGO_TARGET_DIR"]) / profile / ("horizon-client.exe" if os.name == "nt" else "horizon-client")
    print(exe)


def cmd_package(_):
    """The launcher's native mod file: horizon-native.json and the bundle's .rs and .toml files."""
    check_pins()
    mod_id = MANIFEST["mod"]["id"]
    version = MANIFEST["mod"]["version"]
    manifest = {
        "schema": 1,
        "id": mod_id,
        "version": version,
        "game": COMPAT["game"],
        "horizon": COMPAT["horizon_revision"],
        "mixins": COMPAT["horizon_mixins_revision"],
    }
    TARGET.mkdir(exist_ok=True)
    path = TARGET / f"{mod_id}-{version}.hnmod"
    # Fixed dates and modes: the same sources always make the same file (and SHA-256).
    def entry(name):
        info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = 0o644 << 16
        return info

    with zipfile.ZipFile(path, "w") as z:
        z.writestr(entry("horizon-native.json"), json.dumps(manifest, indent=2) + "\n")
        for f in sorted(BUNDLE.iterdir()):
            if f.suffix not in (".rs", ".toml"):
                sys.exit(f"{f.name}: a native bundle holds only .rs and .toml files")
            z.writestr(entry(f"mixins/{mod_id}/{f.name}"), f.read_bytes())
    print(path)


def launcher_constants(src):
    """What the launcher at the pinned Horizon puts in a native build's key: its Rust, profile and features."""
    rust = (src / "launcher" / "src" / "rust_toolchain.rs").read_text()
    native = (src / "launcher" / "src" / "native_mods.rs").read_text()
    def const(text, name):
        found = re.search(rf'const {name}: &str = "([^"]*)";', text)
        if not found:
            sys.exit(f"the launcher has no {name}: its native build changed")
        return found.group(1)
    if const(native, "MIXINS_REV") != COMPAT["horizon_mixins_revision"]:
        sys.exit("compatibility.json's horizon-mixins is not the launcher's MIXINS_REV")
    return {
        "rust": const(rust, "VERSION"),
        "profile": const(native, "PROFILE"),
        "features": const(native, "FEATURES"),
    }


def native_key(consts):
    """The launcher's version_for (launcher/src/native_mods.rs) for this one bundle, given as a folder:
    native-<7 digits of the revision>-<12 hex digits of SHA-256 over the inputs and the bundle's files>.
    install.sh computes the same in sh; tests check they agree."""
    rev = COMPAT["horizon_revision"]
    h = hashlib.sha256()
    for part in [COMPAT["horizon_mixins_revision"], consts["profile"], consts["features"], consts["rust"], rev]:
        h.update(part.encode() + b"\0")
    h.update(MANIFEST["mod"]["id"].encode() + b"\0")
    files = sorted((f.relative_to(BUNDLE).as_posix(), f) for f in BUNDLE.rglob("*") if f.is_file())
    for name, f in files:
        data = f.read_bytes()
        h.update(name.encode() + len(data).to_bytes(8, "little") + data)
    return f"native-{rev[:7]}-{h.hexdigest()[:12]}"


def cmd_native_key(args):
    """The name, checked against install.sh's own computation of it (which the prebuilt game's install uses)."""
    check_pins()
    consts = launcher_constants(source(args.horizon))
    version = native_key(consts)
    theirs = subprocess.run(
        ["sh", ROOT / "install.sh", "--key", BUNDLE, COMPAT["horizon_mixins_revision"], consts["profile"],
         consts["features"], consts["rust"], COMPAT["horizon_revision"]],
        capture_output=True, text=True, check=True).stdout.strip()
    if theirs != version:
        sys.exit(f"install.sh names the build {theirs}, the launcher's rule {version}")
    print(version)


def cmd_prebuilt(args):
    """The modded game built as the launcher builds it (release profile, same horizon-mixins, same bundle),
    packed with that bundle and its version name, for install.sh to put in place instead of compiling."""
    check_pins()
    src = source(args.horizon)
    consts = launcher_constants(src)
    if consts["profile"] != "release":
        sys.exit(f"the launcher builds with {consts['profile']}, not release")
    args.release = True
    out = stage(args.horizon)
    run(["cargo", "build", "--locked", "--profile", "release", "-p", "horizon-client", "--bin", "horizon-client"],
        cwd=out, env=cargo_env())
    exe_name = "horizon-client.exe" if os.name == "nt" else "horizon-client"
    exe = Path(cargo_env()["CARGO_TARGET_DIR"]) / "release" / exe_name
    target = args.target or subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout.split("host: ")[1].split()[0]
    path = pack_prebuilt(exe, target, src, consts)
    log(f"{native_key(consts)} for {target}")
    print(path)


def pack_prebuilt(exe, target, src, consts):
    """target/horizon-studio-game-<target>.tar.gz: the game program, the bundle it was built from, the
    launcher's name for that build and what install.sh hashes into it, and Horizon's license."""
    exe_name = "horizon-client.exe" if exe.suffix == ".exe" else "horizon-client"
    version = native_key(consts)
    about = {
        "horizon": COMPAT["horizon_revision"],
        "horizon_mixins": COMPAT["horizon_mixins_revision"],
        "rust": consts["rust"],
        "mods": [{"id": MANIFEST["mod"]["id"], "version": MANIFEST["mod"]["version"]}],
        "prebuilt": target,
    }
    path = TARGET / f"horizon-studio-game-{target}.tar.gz"
    with tarfile.open(path, "w:gz") as tar:
        def add_bytes(name, data, mode=0o644):
            info = tarfile.TarInfo(name)
            info.size, info.mode = len(data), mode
            tar.addfile(info, io.BytesIO(data))
        tar.add(exe, arcname=exe_name, filter=lambda i: (setattr(i, "mode", 0o755), i)[1])
        add_bytes("native-version", (version + "\n").encode())
        # What install.sh hashes into the name, one per line (it takes the user's launcher's Rust).
        inputs = [COMPAT["horizon_mixins_revision"], consts["profile"], consts["features"], consts["rust"],
                  COMPAT["horizon_revision"], COMPAT["game"]]
        add_bytes("key-inputs", ("\n".join(inputs) + "\n").encode())
        add_bytes("native-mods.json", (json.dumps(about) + "\n").encode())
        add_bytes("LICENSE-Horizon", (src / "LICENSE").read_bytes())
        for f in sorted(BUNDLE.iterdir()):
            add_bytes(f"bundle/{MANIFEST['mod']['id']}/{f.name}", f.read_bytes())
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["test", "check", "build", "package", "native-key", "prebuilt", "all"])
    parser.add_argument("--horizon", help="a Horizon checkout holding the pinned commit")
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--target", help="prebuilt: the target triple in the file's name (default: rustc's host)")
    args = parser.parse_args()
    if args.command == "all":
        cmd_test(args)
        cmd_check(args)
        cmd_package(args)
    else:
        globals()[f"cmd_{args.command.replace('-', '_')}"](args)


if __name__ == "__main__":
    main()
