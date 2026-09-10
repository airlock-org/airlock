from __future__ import annotations

import os
import subprocess
from pathlib import Path


def format_command(
    command: tuple[str, ...], repo_root: Path, log_dir: Path
) -> list[str]:
    return [arg.format(root=str(repo_root), log_dir=str(log_dir)) for arg in command]


def write_log(path: Path, result: subprocess.CompletedProcess[str]) -> None:
    path.write_text(
        "".join(
            (
                f"$ {' '.join(result.args)}\n",
                f"exit_code={result.returncode}\n\n",
                "stdout:\n",
                result.stdout,
                "\n\nstderr:\n",
                result.stderr,
            )
        ),
        encoding="utf-8",
    )


def run_local_command(
    *,
    name: str,
    command: tuple[str, ...],
    repo_root: Path,
    nixome_log_dir: Path,
) -> Path:
    # Each test gets its own mutable artifact directory, while Nix itself keeps
    # using the shared store configured by lifecycle/misc/tools/scripts/lib/nixome-env.sh.
    test_log_dir = nixome_log_dir / name
    test_log_dir.mkdir(parents=True, exist_ok=True)
    formatted_command = format_command(command, repo_root, test_log_dir)
    log_path = test_log_dir / "pytest-command.log"
    env = os.environ.copy()
    env["NIXOME_LOG_DIR"] = str(test_log_dir)

    result = subprocess.run(
        formatted_command,
        cwd=repo_root,
        env=env,
        check=False,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    write_log(log_path, result)

    assert (
        result.returncode == 0
    ), f"{name} failed with exit {result.returncode}; log: {log_path}"
    return log_path
