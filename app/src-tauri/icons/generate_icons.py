#!/usr/bin/env python3
"""Generate NexusOS Tauri icon assets from the base SVG."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def main() -> int:
    icons_dir = Path(__file__).resolve().parent
    app_dir = icons_dir.parent.parent
    svg_path = icons_dir / "nexusos-mark.svg"

    if not svg_path.exists():
        print(f"error: missing base SVG: {svg_path}", file=sys.stderr)
        return 1

    cmd = ["npm", "run", "tauri", "icon", "src-tauri/icons/nexusos-mark.svg"]
    print("running:", " ".join(cmd))
    subprocess.run(cmd, cwd=app_dir, check=True)

    required = [
        "32x32.png",
        "128x128.png",
        "128x128@2x.png",
        "icon.ico",
        "icon.png",
        "Square30x30Logo.png",
        "Square44x44Logo.png",
        "Square71x71Logo.png",
        "Square89x89Logo.png",
        "Square107x107Logo.png",
        "Square142x142Logo.png",
        "Square150x150Logo.png",
        "Square284x284Logo.png",
        "Square310x310Logo.png",
        "StoreLogo.png",
    ]

    missing = [name for name in required if not (icons_dir / name).exists()]
    if missing:
        print("error: icon generation incomplete, missing:", file=sys.stderr)
        for name in missing:
            print(f"  - {name}", file=sys.stderr)
        return 1

    print("icon generation completed; required assets are present")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
