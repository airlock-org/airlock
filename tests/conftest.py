from __future__ import annotations

import json
import os
import subprocess
import time
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
SESSION_START_KEY = "nixome_session_start"
RUNTIME_REPORTS: dict[str, dict[str, object]] = {}


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _load_nixome_env(repo_root: Path) -> None:
    if os.environ.get("NIXOME_PYTEST_ENV_LOADED") == "1":
        return

    # Load the same guarded environment that Make/script entry points use.
    # This should point Nix at the shared Nixome store, not the host /nix/store,
    # unless the caller explicitly set NIXOME_USE_HOST_NIX_STORE=1.
    result = subprocess.run(
        [
            "bash",
            "-c",
            'source "$1"; env -0',
            "bash",
            str(repo_root / "runtime/tools/scripts/lib/nixome-env.sh"),
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


def _log_dir(repo_root: Path) -> Path:
    return Path(os.environ.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))


def pytest_sessionstart(session: pytest.Session) -> None:
    RUNTIME_REPORTS.clear()
    if hasattr(session.config, "workerinput"):
        return

    repo_root = _repo_root()
    _load_nixome_env(repo_root)

    session.config.stash[SESSION_START_KEY] = time.time()

    log_dir = _log_dir(repo_root)
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
        # Warm common immutable inputs once in the shared Nixome store. Per-test
        # mutable artifacts still live under each test's own NIXOME_LOG_DIR.
        _run_setup_command(
            log_dir,
            "warm-erofs-utils",
            ["nix", "build", "--no-link", "--print-out-paths", "nixpkgs#erofs-utils"],
        )


def pytest_configure(config: pytest.Config) -> None:
    _load_nixome_env(_repo_root())
    if not hasattr(config, "workerinput"):
        config.stash.setdefault(SESSION_START_KEY, time.time())


def pytest_runtest_logreport(report: pytest.TestReport) -> None:
    if report.when not in {"setup", "call", "teardown"}:
        return

    entry = RUNTIME_REPORTS.setdefault(
        report.nodeid,
        {
            "nodeid": report.nodeid,
            "outcome": "passed",
            "setup_seconds": 0.0,
            "call_seconds": 0.0,
            "teardown_seconds": 0.0,
        },
    )
    entry[f"{report.when}_seconds"] = report.duration
    if report.failed:
        entry["outcome"] = "failed"
    elif report.skipped and entry["outcome"] == "passed":
        entry["outcome"] = "skipped"


def pytest_testnodedown(node, error) -> None:
    worker_reports = node.workeroutput.get("nixome_runtime_reports", {})
    RUNTIME_REPORTS.update(worker_reports)


def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    if hasattr(session.config, "workerinput"):
        session.config.workeroutput["nixome_runtime_reports"] = RUNTIME_REPORTS
        return
    if session.config.option.collectonly:
        return

    repo_root = _repo_root()
    log_dir = _log_dir(repo_root)
    log_dir.mkdir(parents=True, exist_ok=True)
    started_at = session.config.stash.get(SESSION_START_KEY, time.time())
    finished_at = time.time()
    tests = []
    for report in RUNTIME_REPORTS.values():
        total_seconds = (
            report["setup_seconds"]
            + report["call_seconds"]
            + report["teardown_seconds"]
        )
        tests.append({**report, "total_seconds": total_seconds})
    tests.sort(key=lambda item: item["total_seconds"], reverse=True)

    latest = {
        "schema": "nixome.pytest-runtimes.v1",
        "exitstatus": exitstatus,
        "started_at": started_at,
        "finished_at": finished_at,
        "duration_seconds": finished_at - started_at,
        "test_count": len(tests),
        "tests": tests,
    }
    (log_dir / "test-runtimes-latest.json").write_text(
        json.dumps(latest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    with (log_dir / "test-runtimes-latest.tsv").open("w", encoding="utf-8") as handle:
        handle.write("seconds\toutcome\tnodeid\n")
        for test in tests:
            handle.write(
                f"{test['total_seconds']:.6f}\t{test['outcome']}\t{test['nodeid']}\n"
            )

    history_entry = {
        "schema": "nixome.pytest-runtime-history.v1",
        "finished_at": finished_at,
        "exitstatus": exitstatus,
        "duration_seconds": latest["duration_seconds"],
        "test_count": len(tests),
        "tests": tests,
    }
    with (log_dir / "test-runtimes-history.jsonl").open(
        "a", encoding="utf-8"
    ) as handle:
        handle.write(json.dumps(history_entry, sort_keys=True) + "\n")


@pytest.fixture
def repo_root() -> Path:
    return _repo_root()


@pytest.fixture
def nixome_log_dir(repo_root: Path) -> Path:
    log_dir = Path(os.environ.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)
    return log_dir
