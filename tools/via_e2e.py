#!/usr/bin/env python3
"""ViaHorizon end to end: the modded game's network code joins two fake 1.8
servers through the real ViaProxy (downloaded and run by ViaHorizon itself).

    python3 tools/via_e2e.py [--version 1.8.x] [--keep]

Needs: the staged modded game (tools/studio.py check makes it), Java 17+ or
network access to download it, Python's `cryptography`. It builds
tools/via_probe.rs as an example of the staged horizon-client (fast profile),
starts tools/fake_server_1_8.py offline and online, and checks every step:
ViaProxy starts, the server list's ping goes through it, an offline and an
online join reach Join Game as 26.3, ViaProxy's sign-in request is answered,
and without an account the online join is refused with Horizon's reason.
"""
import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STAGE = ROOT / "target" / "stage"


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", action="append", help="versions to join as (default: 1.8.x, then auto)")
    ap.add_argument("--data", help="ViaHorizon's folder (kept: downloads are reused)",
                    default=str(ROOT / "target" / "via-e2e"))
    args = ap.parse_args()
    example = STAGE / "crates" / "horizon-client" / "examples" / "hstudio_via_probe.rs"
    if not example.parent.is_dir():
        sys.exit("No staged game: run python3 tools/studio.py check first")
    shutil.copy(ROOT / "tools" / "via_probe.rs", example)
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target" / "game"))
    print("building the probe...", flush=True)
    subprocess.run(["cargo", "build", "--locked", "--profile", "fast", "-p", "horizon-client", "--example",
                    "hstudio_via_probe"], cwd=STAGE, env=env, check=True)
    probe = Path(env["CARGO_TARGET_DIR"]) / "fast" / "examples" / "hstudio_via_probe"
    ok = True
    for version in args.version or ["1.8.x", "auto"]:
        print(f"=== joining as {version} ===", flush=True)
        ok &= round_trip(probe, args.data, version)
    sys.exit(0 if ok else 1)


def round_trip(probe, data, version):
    offline_port, online_port = free_port(), free_port()
    logs = Path(tempfile.mkdtemp(prefix="via-e2e-"))
    servers = []
    for port, flag, name in [(offline_port, [], "offline"), (online_port, ["--online"], "online")]:
        out = open(logs / f"{name}.log", "w")
        servers.append((subprocess.Popen([sys.executable, ROOT / "tools" / "fake_server_1_8.py", str(port), *flag],
                                         stdout=out, stderr=subprocess.STDOUT), logs / f"{name}.log"))
    try:
        result = subprocess.run([probe, data, version, f"127.0.0.1:{offline_port}",
                                 f"127.0.0.1:{online_port}"], capture_output=True, text=True, timeout=600)
    finally:
        for p, _ in servers:
            p.terminate()
    print(result.stdout)
    if result.stderr.strip():
        print("stderr:", result.stderr[-3000:])
    seen = {}
    for p, path in servers:
        seen[path.stem] = path.read_text()
        print(f"--- {path.stem} server ---\n{seen[path.stem]}")
    out = result.stdout
    checks = {
        "ViaProxy started": "READY " in out,
        "the server list's ping was answered": "PING " in out,
        "offline join reached Join Game": "JOINED offline" in out,
        "online join reached Join Game": "JOINED online" in out,
        "the sign-in request was answered": "SIGNED-IN [\"" in out,
        "without an account, Horizon's reason": "REFUSED This server is in online mode" in out,
        "the offline server saw protocol 47": '"protocol": 47, "host": "127.0.0.1", "port": %d, "next": 2' % offline_port
        in seen.get("offline", ""),
        "the online server's encryption worked": "ENCRYPTION ok" in seen.get("online", ""),
    }
    if version != "auto":
        checks["the server list saw a joinable server"] = '"protocol":777' in out.replace(" ", "")
    for name, ok in checks.items():
        print(("PASS " if ok else "FAIL ") + name)
    return all(checks.values()) and result.returncode == 0


if __name__ == "__main__":
    main()
