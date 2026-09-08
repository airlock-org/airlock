from __future__ import annotations

import os
from pathlib import Path

import pytest


@pytest.fixture
def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


@pytest.fixture
def nixome_log_dir(repo_root: Path) -> Path:
    log_dir = Path(os.environ.get("NIXOME_LOG_DIR", repo_root / ".nixome-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)
    return log_dir
