"""Real systemd job regression with inert user units, never real Qubes/GPU units."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

import pytest


@pytest.mark.skipif(
    os.environ.get("AIRLOCK_RUN_SYSTEMD_ORDER_TEST") != "1",
    reason="opt-in inert user-systemd unit test; set AIRLOCK_RUN_SYSTEMD_ORDER_TEST=1",
)
@pytest.mark.parametrize(
    "dependency,setup_ok,daemon_ok,expected",
    [
        ("Requires", True, True, False),  # Reproduce the original cancelled job.
        ("Wants", True, True, True),
        ("Wants", False, True, False),
        ("Wants", True, False, False),
    ],
)
def test_bootstrap_daemon_restart(tmp_path, dependency, setup_ok, daemon_ok, expected):
    systemctl = shutil.which("systemctl")
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not systemctl or not runtime:
        pytest.skip("requires a local systemd user manager")

    def ctl(*args):
        return subprocess.run(
            [systemctl, "--user", *args],
            capture_output=True,
            text=True,
            timeout=20,
            check=False,
        )

    if ctl("show-environment").returncode:
        pytest.skip("systemd user manager is unavailable")
    prefix = "airlock-gpu-order-test-" + uuid.uuid4().hex
    daemon, setup, guest = [
        f"{prefix}-{role}.service" for role in ("daemon", "setup", "guest")
    ]
    unit_dir = Path(runtime) / "systemd/user"
    unit_dir.mkdir(parents=True, exist_ok=True)
    marker = tmp_path / "started"
    setup_script = tmp_path / "setup.py"
    setup_script.write_text(
        "import subprocess, sys\n"
        f"subprocess.run({[systemctl, '--user', 'stop', daemon]!r}, check=True)\n"
        + (
            f"subprocess.run({[systemctl, '--user', 'start', daemon]!r}, check=True)\n"
            if daemon_ok
            else ""
        )
        + f"sys.exit({0 if setup_ok else 1})\n"
    )
    start_script = tmp_path / "start.py"
    start_script.write_text(
        f"from pathlib import Path\nPath({str(marker)!r}).write_text('started\\n')\n"
    )
    definitions = {
        daemon: f"[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart={shutil.which('true')}\n",
        setup: f"[Unit]\nWants={daemon}\nAfter={daemon}\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart={sys.executable} {setup_script}\n",
        guest: f"[Unit]\nRequires={setup}\n{dependency}={daemon}\nAfter={setup} {daemon}\n[Service]\nType=oneshot\nRemainAfterExit=yes\nRestart=no\nExecStartPre={systemctl} --user is-active --quiet {daemon}\nExecStart={sys.executable} {start_script}\n",
    }
    try:
        for name, text in definitions.items():
            (unit_dir / name).write_text(text)
        assert ctl("daemon-reload").returncode == 0
        result = ctl("start", guest)
        # The old job can return cancelled while bootstrap is still running.
        deadline = time.monotonic() + 10
        while (
            ctl("show", setup, "-p", "ActiveState", "--value").stdout.strip()
            == "activating"
        ):
            assert time.monotonic() < deadline, "mock bootstrap did not settle"
            time.sleep(0.05)
        states = ctl(
            "show",
            daemon,
            setup,
            guest,
            "-p",
            "Id",
            "-p",
            "ActiveState",
            "-p",
            "Result",
        ).stdout
        logs = (
            Path(os.environ.get("AIRLOCK_LOG_DIR", ".airlock-logs"))
            / "gui-gpu-boot-order"
        )
        logs.mkdir(parents=True, exist_ok=True)
        (logs / f"{dependency}-{setup_ok}-{daemon_ok}.json").write_text(
            json.dumps(
                {
                    "dependency": dependency,
                    "setup_ok": setup_ok,
                    "daemon_ok": daemon_ok,
                    "returncode": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "states": states,
                    "guest_started": marker.exists(),
                },
                indent=2,
            )
            + "\n"
        )
        assert marker.exists() is expected, result.stdout + result.stderr + states
        assert (result.returncode == 0) is expected
    finally:
        # Only our uniquely named inert units; never system services or VMs.
        ctl("stop", guest, setup, daemon)
        ctl("reset-failed", guest, setup, daemon)
        for name in definitions:
            (unit_dir / name).unlink(missing_ok=True)
        ctl("daemon-reload")
