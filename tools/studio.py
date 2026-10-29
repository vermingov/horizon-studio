#!/usr/bin/env python3
"""Build Horizon Studio into Horizon, and package it for the launcher.

    python3 tools/studio.py test       # the studio's own tests (no game needed)
    python3 tools/studio.py check      # apply the bundle on the pinned Horizon, cargo check the game
    python3 tools/studio.py build      # ... and build it (fast profile); prints the game's path
    python3 tools/studio.py package    # target/horizon_studio-<version>.hnmod, the launcher's native mod file
    python3 tools/studio.py all        # test, check, package

    --horizon PATH   use this Horizon checkout (it needs the pinned commit) instead of GitHub's archive
    --release        build with the release profile (as the launcher does) instead of `fast`

Horizon's source is the commit pinned in compatibility.json; horizon-mixins is built once at its pinned commit.
Everything goes under target/. The bundle is mixins/horizon_studio/; the modules' tests run on their own with
cargo test, and examples.rs is generated from examples/*.lua (tools/gen_examples.py).
"""
import argparse
import io
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
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("horizon-native.json", json.dumps(manifest, indent=2) + "\n")
        for f in sorted(BUNDLE.iterdir()):
            if f.suffix not in (".rs", ".toml"):
                sys.exit(f"{f.name}: a native bundle holds only .rs and .toml files")
            z.write(f, f"mixins/{mod_id}/{f.name}")
    print(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["test", "check", "build", "package", "all"])
    parser.add_argument("--horizon", help="a Horizon checkout holding the pinned commit")
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    if args.command == "all":
        cmd_test(args)
        cmd_check(args)
        cmd_package(args)
    else:
        globals()[f"cmd_{args.command}"](args)


if __name__ == "__main__":
    main()
