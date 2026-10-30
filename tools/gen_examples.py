#!/usr/bin/env python3
"""Writes mixins/horizon_studio/examples.rs from examples/*.lua.

The bundle's modules cannot use include_str!, so the bundled examples are
embedded as string constants; tests/examples.rs fails when this file is stale.
Run: python3 tools/gen_examples.py
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORDER = [
    "hello", "coords", "clock", "compass", "frame_graph", "zoom", "notes", "night_watch", "ore_finder", "miner",
    "waypoints", "selection", "playground", "outfit", "uikit", "uikit_demo", "viahorizon",
]


def header(src, key):
    for line in src.splitlines():
        line = line.strip()
        if not line.startswith("--"):
            break
        m = re.match(r"--\s*" + key + r":\s*(.*)", line)
        if m:
            return m.group(1).strip()
    return ""


def raw(s):
    hashes = "#"
    while '"' + hashes in s:
        hashes += "#"
    return f'r{hashes}"{s}"{hashes}'


def main():
    files = {p.stem: p.read_text() for p in (ROOT / "examples").glob("*.lua")}
    missing = set(files) - set(ORDER)
    if missing:
        raise SystemExit(f"add {sorted(missing)} to ORDER in tools/gen_examples.py")
    out = [
        "// The bundled example mods, from examples/*.lua by tools/gen_examples.py:",
        "// edit those files and run it again (tests/examples.rs checks this file).",
        "",
        "pub struct Example {",
        "    pub id: &'static str,",
        "    pub title: &'static str,",
        "    pub description: &'static str,",
        "    pub source: &'static str,",
        "}",
        "",
    ]
    entries = []
    for stem in ORDER:
        src = files[stem]
        entries.append(
            "    Example {\n"
            f"        id: {raw(stem)},\n"
            f"        title: {raw(header(src, 'name'))},\n"
            f"        description: {raw(header(src, 'description'))},\n"
            f"        source: {raw(src)},\n"
            "    },"
        )
    out.append("/// Installed on a first run.")
    out.append("pub const WELCOME: &Example = &EXAMPLES[0];")
    out.append("")
    out.append("pub const EXAMPLES: &[Example] = &[")
    out.extend(entries)
    out.append("];")
    (ROOT / "mixins/horizon_studio/examples.rs").write_text("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
