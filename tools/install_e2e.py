#!/usr/bin/env python3
"""install.sh (and install.ps1) end to end, against fake launchers that name and find native builds as Horizon's do.

The prebuilt game (packed by tools/studio.py, with a stand-in program) is served from a local
web server. Each fake launcher follows the real launcher/src/native_mods.rs of its time: how
it reads a bundle folder or a registry-style one (horizon-native.json beside mixins/<id>/),
which horizon-mixins commit goes in a build's name, its profile and its Rust. install.sh must
put the game where each one looks, so that its native-build only selects it, never compiles:

- old: the launcher at the pinned Horizon (horizon-mixins a2af5c0, always the release profile,
  no --profile option);
- new: a later one (another horizon-mixins, the fast profile by default, --profile);
- new with another Rust: the name takes the launcher's own Rust.

install.ps1 runs the same cases where there is a PowerShell (pwsh), on any system: told it is
on a 64-bit Windows (PROCESSOR_ARCHITECTURE), it installs the Windows prebuilt game (a .zip, with
horizon-client.exe) in the same fake launchers.

Run: python3 tools/install_e2e.py (install.sh: Linux on x86_64, or macOS on Apple silicon, the
platforms with a prebuilt game it installs; install.ps1: wherever pwsh is).
"""
import argparse
import hashlib
import http.server
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

WINDOWS = "x86_64-pc-windows-msvc"

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio  # noqa: E402

ROOT = studio.ROOT
FAKE = r'''#!/usr/bin/env python3
# A fake horizon-launcher: native-build names and finds builds as the real one does.
import hashlib, json, os, sys, tomllib
mode, state = os.environ["FAKE_MODE"], os.environ["FAKE_STATE"]
data = os.environ["HORIZON_LAUNCHER_HOME"]
mixins_rev = {"old": "a2af5c0dd4117d51ecd70066ba7a00b4a2c10dab",
              "new": "4342a0bfbe71d4d14185528d64e496d6022f16a1"}[mode]
args = sys.argv[1:]
with open(os.path.join(state, "calls"), "a") as log:
    log.write(" ".join(args) + "\n")

def files(d):
    out = []
    for root, _, names in os.walk(d):
        for n in names:
            p = os.path.join(root, n)
            out.append((os.path.relpath(p, d).replace(os.sep, "/"), p))
    return sorted(out)

def bundle(d, mixins):
    toml = tomllib.load(open(os.path.join(d, "horizon-mixin.toml"), "rb"))
    return {"id": toml["mod"]["id"], "version": toml["mod"]["version"], "dir": d,
            "horizon": toml.get("compatibility", {}).get("horizon_revision"), "mixins": mixins}

def bundle_in(d):
    if os.path.isfile(os.path.join(d, "horizon-mixin.toml")):
        return bundle(d, None)
    path = os.path.join(d, "horizon-native.json")
    if not os.path.isfile(path):
        return None
    native = json.load(open(path))
    found = bundle(os.path.join(d, "mixins", native["id"]), native.get("mixins"))
    if found["horizon"] and native.get("horizon") and found["horizon"] != native["horizon"]:
        sys.exit("horizon-native.json and horizon-mixin.toml pin different Horizon commits")
    found["horizon"] = found["horizon"] or native.get("horizon")
    return found

def version_for(revision, mixins, profile, bundles):
    h = hashlib.sha256()
    for part in [mixins, profile, "horizon-client/mods", os.environ.get("FAKE_RUST", "1.99.1"), revision]:
        h.update(part.encode() + b"\0")
    for b in sorted(bundles, key=lambda b: b["id"]):
        h.update(b["id"].encode() + b"\0")
        for name, path in files(b["dir"]):
            body = open(path, "rb").read()
            h.update(name.encode() + len(body).to_bytes(8, "little") + body)
    return f"native-{revision[:7]}-{h.hexdigest()[:12]}"

if args == ["--json", "rust", "status"]:
    print(json.dumps({"pinned": os.environ.get("FAKE_RUST", "1.99.1"), "installed": []}, indent=2))
elif args[:1] == ["native-build"]:
    rest = args[1:]
    if "--help" in rest:
        print("Build the game with native mods\n\nUsage: horizon-launcher native-build [OPTIONS] [FOLDER]")
        print("      --trust-native-code")
        if mode == "new":
            print("      --profile <PROFILE>  The Cargo profile: fast (the default) or release")
        sys.exit(0)
    if "--trust-native-code" not in rest:
        sys.exit("Native mods run with full access to this computer.")
    rest.remove("--trust-native-code")
    profile = "release"
    if "--profile" in rest:
        if mode == "old":
            sys.stderr.write("error: unexpected argument '--profile' found\n")
            sys.exit(2)
        i = rest.index("--profile")
        profile = rest[i + 1]
        del rest[i:i + 2]
    elif mode == "new":
        profile = "fast"
    base = None
    if "--version" in rest:
        i = rest.index("--version")
        base = rest[i + 1]
        del rest[i:i + 2]
    folder = rest[0]
    found = bundle_in(folder)
    bundles = [found] if found else [b for b in (bundle_in(os.path.join(folder, d)) for d in sorted(os.listdir(folder))) if b]
    pins = sorted({b["mixins"] for b in bundles if b["mixins"]})
    # With one pin, both launchers take it (agreed); with none, their own horizon-mixins.
    mixins = pins[0] if len(pins) == 1 else mixins_rev
    revision = bundles[0]["horizon"]
    version = version_for(revision, mixins, profile, bundles)
    there = os.path.join(data, "versions", version)
    if base and not os.path.isfile(os.path.join(data, "versions", base, ".installed")):
        sys.exit(f"{base} is not installed")
    if os.path.isfile(os.path.join(there, ".installed")):
        secs = 0
    else:
        with open(os.path.join(state, "compiled"), "a") as log:
            log.write(version + "\n")
        os.makedirs(there, exist_ok=True)
        open(os.path.join(there, ".installed"), "w").close()
        secs = 1234
    with open(os.path.join(state, "selected"), "w") as out:
        out.write(version)
    print(f"{version} built and selected for its instance ({secs} s).")
else:
    sys.exit(f"the fake launcher has no {' '.join(args)}")
'''


def platform_target():
    machine = platform.machine().lower()
    if sys.platform.startswith("linux") and machine in ("x86_64", "amd64"):
        return "x86_64-unknown-linux-gnu"
    if sys.platform == "darwin" and machine == "arm64":
        return "aarch64-apple-darwin"
    return None


def serve(folder):
    """A quiet local web server for `folder`, as the release's downloads."""
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **k):
            super().__init__(*a, directory=str(folder), **k)

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def prepared(data, tag, game):
    """A Horizon version as the launcher keeps a prepared one."""
    v = data / "versions" / tag
    (v / ".local" / "horizon-client" / game).mkdir(parents=True)
    (v / ".local" / "horizon-client" / game / "horizon-resources.json").write_text("{}\n")
    (v / "research-lock.json").write_text(json.dumps({"version": game}) + "\n")
    (v / "horizon-client").write_text("#!/bin/sh\necho plain horizon\n")
    (v / "assets").mkdir()
    (v / "assets" / "a.png").write_bytes(b"png")
    (v / ".installed").write_text("")


def run_install(installer, env, state, mode, rust):
    """`installer` (install.sh, or install.ps1 through pwsh) against the fake launcher in `mode`,
    which notes what it did in `state`."""
    state.mkdir()
    run_env = dict(env, FAKE_MODE=mode, FAKE_STATE=str(state), FAKE_RUST=rust)
    if installer == "install.sh":
        cmd = ["sh", str(ROOT / "install.sh")]
    else:
        cmd = [shutil.which("pwsh"), "-NoProfile", "-NonInteractive", "-File", str(ROOT / "install.ps1")]
        # A 64-bit Windows, as far as install.ps1 asks; its cache in this home.
        run_env.update(PROCESSOR_ARCHITECTURE="AMD64", LOCALAPPDATA=str(Path(env["HOME"]) / "local"))
        run_env.pop("PROCESSOR_ARCHITEW6432", None)
    return subprocess.run(cmd, env=run_env, capture_output=True, text=True)


def main():
    target = platform_target()
    # Each installer, the target whose prebuilt game it installs, and that game's program.
    installers = []
    if target:
        installers.append(("install.sh", target, "horizon-client"))
    if shutil.which("pwsh"):
        installers.append(("install.ps1", WINDOWS, "horizon-client.exe"))
    else:
        print("install_e2e: no pwsh here; install.ps1 not run")
    if not installers:
        print("install_e2e: no prebuilt game for this platform; skipped")
        return
    work = Path(tempfile.mkdtemp(prefix="hstudio-install-"))
    failures = []
    try:
        # The release: the games packed as the release workflow packs them, with a stand-in program.
        rel = work / "rel"
        rel.mkdir()
        src = work / "horizon-src"
        src.mkdir()
        (src / "LICENSE").write_text("Horizon's license\n")
        consts = {"rust": "1.99.1", "profile": "release", "features": "horizon-client/mods"}
        studio.TARGET = rel
        sums = []
        for _, packed_for, program in installers:
            exe = work / program
            exe.write_text("#!/bin/sh\necho horizon studio\n")
            packed = studio.pack_prebuilt(exe, packed_for, src, consts)
            sums.append(f"{hashlib.sha256(packed.read_bytes()).hexdigest()}  {packed.name}\n")
        (rel / "SHA256SUMS").write_text("".join(sums))
        server = serve(rel)
        url = f"http://127.0.0.1:{server.server_address[1]}"

        fake = work / "horizon-launcher"
        fake.write_text(FAKE)
        fake.chmod(0o755)
        cases = [(i, m, r) for i in installers for m, r in [("old", "1.99.1"), ("new", "1.99.1"), ("new", "1.100.0")]]
        for (installer, _, program), mode, rust in cases:
            exe = work / program
            home = work / f"home-{installer}-{mode}-{rust}"
            data = home / "data"
            prepared(data, "0.2.80", studio.COMPAT["game"])
            env = dict(os.environ, HOME=str(home), XDG_CACHE_HOME=str(home / "cache"),
                       HORIZON_LAUNCHER=str(fake), HORIZON_LAUNCHER_HOME=str(data),
                       HORIZON_STUDIO_RELEASE_URL=url, NO_PROXY="127.0.0.1,localhost",
                       no_proxy="127.0.0.1,localhost")
            # The second run finds the game installed already and only selects it again.
            for attempt in (1, 2):
                state = home / f"state{attempt}"
                out = run_install(installer, env, state, mode, rust)
                said = out.stderr + out.stdout
                label = f"{installer}, {mode} launcher, Rust {rust}, run {attempt}"
                compiled = (state / "compiled").read_text() if (state / "compiled").exists() else ""
                selected = (state / "selected").read_text() if (state / "selected").exists() else ""
                if out.returncode != 0:
                    failures.append(f"{label}: {installer} failed ({out.returncode}):\n{said}")
                    continue
                if compiled:
                    failures.append(f"{label}: the launcher compiled {compiled.strip()} instead of finding the game:\n{said}")
                    continue
                if "Done:" not in said:
                    failures.append(f"{label}: no Done line:\n{said}")
                seeded = data / "versions" / selected
                if not selected or not (seeded / ".installed").is_file():
                    failures.append(f"{label}: nothing selected ({selected!r})")
                    continue
                client = seeded / program
                if not client.is_file() or client.read_text() != exe.read_text():
                    failures.append(f"{label}: {selected} plays something else than the prebuilt game")
                    continue
                about = json.loads((seeded / "native-mods.json").read_text())
                if about.get("base") != "0.2.80" or about.get("horizon") != studio.COMPAT["horizon_revision"]:
                    failures.append(f"{label}: native-mods.json lacks what the launcher reads: {about}")
                if about.get("mods") != [{"id": "horizon_studio", "version": studio.MANIFEST["mod"]["version"]}]:
                    failures.append(f"{label}: native-mods.json names other mods: {about}")
                print(f"ok: {label}: {selected}")
        server.shutdown()
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if failures:
        sys.exit("install_e2e failed:\n" + "\n\n".join(failures))
    print("install_e2e: all passed")


if __name__ == "__main__":
    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args()
    main()
