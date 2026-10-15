from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "airlock-machine/tests"))
from airlock_test_support import plugin  # noqa: E402

pytest_addoption = plugin.pytest_addoption


def pytest_configure(config):
    if not config.pluginmanager.hasplugin("airlock-session"):
        config.pluginmanager.register(plugin, "airlock-session")


@pytest.fixture
def repo_root():
    return Path(__file__).resolve().parents[1]
