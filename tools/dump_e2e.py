#!/usr/bin/env python3
"""saveinstance's world, end to end in the game's own code: tools/dump_probe.rs
writes a world as hstudio_world_dump does (Horizon's SaveDir for a chunk, its
chest and an entity; the bundle's world_files.rs for level.dat, the player, the
void generator, a map, the server's registries and tags as a data pack and the
command tree), then reads it back as Singleplayer would (the world list,
Level::read, horizon_world's RegionFile, SaveDir::load, datapack::discover).

    python3 tools/dump_e2e.py [--profile dev]

Needs the staged modded game (tools/studio.py check makes it). It builds the
probe as an example of the staged horizon-client and runs it in a new folder.
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STAGE = ROOT / "target" / "stage"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--profile", default="dev", help="the cargo profile to build the probe with (default: dev)")
    args = ap.parse_args()
    examples = STAGE / "crates" / "horizon-client" / "examples"
    if not examples.parent.is_dir():
        sys.exit("No staged game: run python3 tools/studio.py check first")
    examples.mkdir(exist_ok=True)
    shutil.copy(ROOT / "tools" / "dump_probe.rs", examples / "hstudio_dump_probe.rs")
    shutil.copy(ROOT / "mixins" / "horizon_studio" / "world_files.rs", examples / "hstudio_world_files.rs")
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target" / "game"))
    print("building the probe...", flush=True)
    subprocess.run(["cargo", "build", "--locked", "--profile", args.profile, "-p", "horizon-client", "--example",
                    "hstudio_dump_probe"], cwd=STAGE, env=env, check=True)
    folder = "debug" if args.profile == "dev" else args.profile
    probe = Path(env["CARGO_TARGET_DIR"]) / folder / "examples" / "hstudio_dump_probe"
    work = Path(tempfile.mkdtemp(prefix="dump-e2e-"))
    try:
        result = subprocess.run([probe, work], capture_output=True, text=True, timeout=600)
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print(result.stdout)
    if result.stderr.strip():
        print("stderr:", result.stderr[-3000:])
    sys.exit(0 if result.returncode == 0 and "ALL OK" in result.stdout else 1)


if __name__ == "__main__":
    main()
