#!/usr/bin/env python3
# reconcile.py -- run from the tetron-tui repo root.
# Checks the automatable constraints (CON-D01..CON-D04) from spec/constraints.py.
# Requirement classes (DEB-001..006) are verified by review and by the release job itself.
import json
import re
import sys
import tomllib
from pathlib import Path

ACTION = "tetronos/tetron-packaging"
WORKFLOW = Path(".github/workflows/release.yml")


def check_deb_toml() -> dict:
    try:
        d = tomllib.loads(Path("packaging/deb.toml").read_text())
    except (OSError, tomllib.TOMLDecodeError) as e:
        return {"valid": False, "error": str(e)}
    files = d.get("files", [])
    ok = (d.get("name") == "tetron-tui"
          and any(f.get("dst") == "usr/bin/tetron-tui" and f.get("mode") == "0755" for f in files)
          and "scripts" not in d
          and "maintainer" not in d)
    return {"valid": ok}


def check_workflow() -> dict:
    t = WORKFLOW.read_text()
    uses = re.findall(r"uses:\s*" + re.escape(ACTION) + r"@(\S+)", t)
    unpinned = [u for u in uses if not re.fullmatch(r"[0-9a-f]{40}", u)]
    raw = all(x in t for x in ("tetron-tui-linux-x86_64", "tetron-tui-linux-aarch64", ".sha256"))
    return {"uses_action": bool(uses), "unpinned": len(unpinned), "refs": uses, "raw_assets": raw}


def check_suite_deps() -> dict:
    """CON-D04: contrib/check-suite-deps.sh, the same guard in every suite client."""
    import subprocess
    r = subprocess.run(["sh", "contrib/check-suite-deps.sh"], capture_output=True, text=True)
    return {"ok": r.returncode == 0, "output": r.stdout.strip()}


if __name__ == "__main__":
    ctx = {"deb_toml": check_deb_toml(), "workflow": check_workflow(), "suite_deps": check_suite_deps()}
    print(json.dumps(ctx, indent=2))
    ok = (ctx["deb_toml"]["valid"] and ctx["workflow"]["uses_action"]
          and ctx["workflow"]["unpinned"] == 0 and ctx["workflow"]["raw_assets"] and ctx["suite_deps"]["ok"])
    sys.exit(0 if ok else 1)
