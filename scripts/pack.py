#!/usr/bin/env python3
"""Pack this mod's files the way docs/api/mod-files.md describes them.

  target/sophisticated_backpacks.hmod   horizon-mod.toml + assets/ + data/
  target/sophisticated_backpacks.hnmod  horizon-native.json + mixins/<id>/...
  dist/sophisticated_backpacks.hmod     both halves in one file (the launcher
                                        reads content and native from it)

The build VPS and the client pack copy the two `target/` files; `dist/` is the
single-file form documented in the README.
"""
import hashlib
import json
import os
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
ID = "sophisticated_backpacks"

BUILD = os.path.join(ROOT, "target")
DIST = os.path.join(ROOT, "dist")


def add_tree(zf, base, prefix="", skip=()):
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = sorted(
            d for d in dirnames if d not in skip and not d.startswith(".")
        )
        for name in sorted(filenames):
            if name.startswith("."):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, base).replace("\\", "/")
            zf.write(full, (prefix + rel) if prefix else rel)


def build_content():
    out = os.path.join(BUILD, ID + ".hmod")
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.write(os.path.join(ROOT, "horizon-mod.toml"), "horizon-mod.toml")
        add_tree(zf, os.path.join(ROOT, "assets"), "assets/")
        add_tree(zf, os.path.join(ROOT, "data"), "data/")
    return out


def build_native():
    out = os.path.join(BUILD, ID + ".hnmod")
    with open(os.path.join(ROOT, "horizon-native.json"), encoding="utf-8") as f:
        native = json.load(f)
    assert native["id"] == ID
    bundle = os.path.join(ROOT, "mixins", ID)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
        info = zipfile.ZipInfo("horizon-native.json", date_time=(2026, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        zf.writestr(info, json.dumps(native, indent=2) + "\n")
        add_tree(zf, bundle, "mixins/" + ID + "/", skip=("target", ".build"))
    return out


def build_combined():
    out = os.path.join(DIST, ID + ".hmod")
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
        for name in ("horizon-mod.toml", "horizon-native.json"):
            zf.write(os.path.join(ROOT, name), name)
        for top in ("assets", "data"):
            add_tree(zf, os.path.join(ROOT, top), top + "/")
        add_tree(
            zf,
            os.path.join(ROOT, "mixins", ID),
            "mixins/" + ID + "/",
            skip=("target", ".build"),
        )
    return out


def main():
    os.makedirs(BUILD, exist_ok=True)
    os.makedirs(DIST, exist_ok=True)
    for path in (build_content(), build_native(), build_combined()):
        with open(path, "rb") as f:
            digest = hashlib.sha256(f.read()).hexdigest()
        print(f"{os.path.relpath(path, ROOT)}  {os.path.getsize(path)} bytes  sha256 {digest}")


if __name__ == "__main__":
    main()
