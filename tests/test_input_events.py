"""Qubes lifecycle coordination for physical input forwarding."""

import importlib.util
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "os/lifecycle/runtime/tools/scripts/input-events.py"
SPEC = importlib.util.spec_from_file_location("input_events", SOURCE)
INPUT_EVENTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INPUT_EVENTS)


class VM:
    def __init__(self, name, running=False):
        self.name = name
        self._running = running

    def is_running(self):
        return self._running


def coordinator(monkeypatch, *, running=False):
    calls = []
    monkeypatch.setattr(
        INPUT_EVENTS.subprocess,
        "run",
        lambda command, check: calls.append((command, check)),
    )
    app = SimpleNamespace(domains={"sys-gui": VM("sys-gui", running)})
    instance = INPUT_EVENTS.InputCoordinator(
        app,
        "sys-gui",
        "airlock-gui-input-connect.service",
        "airlock-gui-input.target",
        "/systemctl",
    )
    return instance, calls


def test_domain_start_requests_one_bounded_coordinator_start(monkeypatch):
    instance, calls = coordinator(monkeypatch)

    instance.handle_event(VM("sys-gui"), "domain-start")

    assert calls == [
        (
            [
                "/systemctl",
                "--no-block",
                "start",
                "airlock-gui-input-connect.service",
            ],
            True,
        )
    ]


def test_domain_stop_releases_input_without_retry(monkeypatch):
    instance, calls = coordinator(monkeypatch)

    instance.handle_event(VM("sys-gui"), "domain-stopped")

    assert calls == [
        (
            [
                "/systemctl",
                "--no-block",
                "stop",
                "airlock-gui-input-connect.service",
                "airlock-gui-input.target",
            ],
            True,
        )
    ]


def test_event_reconnect_recovers_an_already_running_gui_domain(monkeypatch):
    instance, calls = coordinator(monkeypatch, running=True)

    instance.handle_event(None, "connection-established")

    assert len(calls) == 1
    assert calls[0][0][-1] == "airlock-gui-input-connect.service"


def test_unrelated_domains_and_halted_initial_state_do_nothing(monkeypatch):
    instance, calls = coordinator(monkeypatch)

    instance.connect_if_running()
    instance.handle_event(VM("usr-home"), "domain-start")

    assert calls == []
