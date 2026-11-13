#!/usr/bin/env python3
"""Pack one combined .hmod: content at the root, native bundle under mixins/.

The launcher reads `horizon-mod.toml` + `assets/` + `data/` as the content and
`horizon-native.json` + `mixins/<id>/` as the native half (docs/api/mod-files.md).
"""
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NS = "sophisticated_backpacks"
OUT = ROOT / "dist" / f"{NS}.hmod"


def main():
    OUT.parent.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((ROOT / "horizon-native.json").read_text())
    assert manifest["id"] == NS
    with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
        for name in ("horizon-mod.toml", "horizon-native.json"):
            z.write(ROOT / name, name)
        for top in ("assets", "data"):
            for path in sorted((ROOT / top).rglob("*")):
                if path.is_file():
                    z.write(path, path.relative_to(ROOT).as_posix())
        bundle = ROOT / "mixins" / NS
        for path in sorted(bundle.rglob("*")):
            if path.is_file() and "target" not in path.parts:
                z.write(path, (Path("mixins") / NS / path.relative_to(bundle)).as_posix())
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
