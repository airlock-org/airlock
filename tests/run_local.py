#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        [
            "bash",
            "-c",
            'source "$1"; env -0',
            "bash",
            str(repo_root / "lifecycle/runtime/tools/scripts/lib/nixome-env.sh"),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode("utf-8", errors="replace"))
        return result.returncode

    env = os.environ.copy()
    for item in result.stdout.split(b"\0"):
        if not item or b"=" not in item:
            continue
        key, value = item.split(b"=", 1)
        env[key.decode("utf-8", errors="replace")] = value.decode(
            "utf-8", errors="replace"
        )

    log_dir = Path(env.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)

    args = sys.argv[1:]
    if not args or args[0].startswith("-"):
        args = ["tests", *args]

    command = [
        "uv",
        "run",
        "pytest",
        *args,
        "--durations=20",
        "--junitxml",
        str(log_dir / "pytest-junit.xml"),
        "--json-report",
        "--json-report-file",
        str(log_dir / "pytest-report.json"),
        *args,
    ]
    os.chdir(repo_root)
    os.execvpe(command[0], command, env)
    raise AssertionError("unreachable")


if __name__ == "__main__":
    raise SystemExit(main())
