"""Run Next.js while updating its collection from canonical Markdown edits."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

WEBSITE = Path(__file__).resolve().parents[1]
ROOT = WEBSITE.parent
spec = importlib.util.spec_from_file_location(
    "prepare_docs", WEBSITE / "scripts/prepare-docs.py"
)
assert spec and spec.loader
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)


def snapshot() -> dict[Path, tuple[int, int]]:
    manifest = WEBSITE / "docs.config.json"
    paths = [manifest]
    config = json.loads(manifest.read_text())
    paths.extend(
        ROOT / page["source"] for group in config["groups"] for page in group["pages"]
    )
    return {path: (path.stat().st_mtime_ns, path.stat().st_size) for path in paths}


def main() -> int:
    docs.prepare(ROOT)
    previous = snapshot()
    child = subprocess.Popen(
        ["pnpm", "exec", "next", "dev", "--hostname", "127.0.0.1", *sys.argv[1:]],
        cwd=WEBSITE,
        start_new_session=True,
    )
    last_error = None
    try:
        while child.poll() is None:
            time.sleep(1)
            try:
                current = snapshot()
                if current != previous:
                    count = docs.prepare(ROOT)
                    previous = current
                    print(f"Updated {count} documentation pages.", flush=True)
                last_error = None
            except (OSError, ValueError, KeyError) as error:
                if str(error) != last_error:
                    print(
                        f"Documentation update failed: {error}",
                        file=sys.stderr,
                        flush=True,
                    )
                    last_error = str(error)
        return child.returncode
    except KeyboardInterrupt:
        return 0
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()


if __name__ == "__main__":
    raise SystemExit(main())
