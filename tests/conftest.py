from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

ENV_KEYS = {
    "NIX_CONFIG",
    "NIXOME_DATA_ROOT",
    "NIXOME_LOG_DIR",
    "NIXOME_NIX_CONFIGURED",
    "NIXOME_NIX_ROOT",
    "NIXOME_NIX_STORE",
    "NIXOME_QUBES_CACHE_DIR",
    "NIXOME_RESULT_LINK_DIR",
    "NIXOME_USE_HOST_NIX_STORE",
    "NIXOME_VM_DIR",
    "TMPDIR",
    "XDG_CACHE_HOME",
}


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _load_nixome_env(repo_root: Path) -> None:
    if os.environ.get("NIXOME_PYTEST_ENV_LOADED") == "1":
        return

    result = subprocess.run(
        [
            "bash",
            "-c",
            'source "$1"; env -0',
            "bash",
            str(repo_root / "scripts/lib/nixome-env.sh"),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        pytest.exit(f"failed to initialize Nixome test environment: {message}", 1)

    for item in result.stdout.split(b"\0"):
        if not item or b"=" not in item:
            continue
        key, value = item.split(b"=", 1)
        name = key.decode("utf-8", errors="replace")
        if name in ENV_KEYS:
            os.environ[name] = value.decode("utf-8", errors="replace")
    os.environ["NIXOME_PYTEST_ENV_LOADED"] = "1"


def _run_setup_command(log_dir: Path, name: str, command: list[str]) -> None:
    log_path = log_dir / f"pytest-session-{name}.log"
    result = subprocess.run(
        command,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    log_path.write_text(
        "".join(
            (
                f"$ {' '.join(command)}\n",
                f"exit_code={result.returncode}\n\n",
                "stdout:\n",
                result.stdout,
                "\n\nstderr:\n",
                result.stderr,
            )
        ),
        encoding="utf-8",
    )
    if result.returncode != 0:
        pytest.exit(f"global test setup command failed; log: {log_path}", 1)


def pytest_sessionstart(session: pytest.Session) -> None:
    if hasattr(session.config, "workerinput"):
        return

    repo_root = _repo_root()
    _load_nixome_env(repo_root)

    log_dir = Path(os.environ.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)
    env_report = {
        key: os.environ[key]
        for key in sorted(ENV_KEYS)
        if key in os.environ and key != "NIX_CONFIG"
    }
    env_report["nix_configured"] = bool(os.environ.get("NIX_CONFIG"))
    (log_dir / "pytest-session-env.json").write_text(
        json.dumps(env_report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    if session.config.option.collectonly:
        return

    if os.environ.get("NIXOME_PYTEST_WARM_STORE", "1") == "1":
        _run_setup_command(
            log_dir,
            "warm-erofs-utils",
            ["nix", "build", "--no-link", "--print-out-paths", "nixpkgs#erofs-utils"],
        )


def pytest_configure(config: pytest.Config) -> None:
    _load_nixome_env(_repo_root())


@pytest.fixture
def repo_root() -> Path:
    return _repo_root()


@pytest.fixture
def nixome_log_dir(repo_root: Path) -> Path:
    log_dir = Path(os.environ.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)
    return log_dir
