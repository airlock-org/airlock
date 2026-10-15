"""Execute parent orchestration boundaries with inert VM and Nix commands."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


@pytest.mark.local
@pytest.mark.parametrize("name", ["run-nested-gui-smoke-static"])
def test_local_check(name, repo_root, airlock_log_dir):
    """Retain the named selector for shell syntax, without source inventories."""
    for relative in (
        "lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh",
        "lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh",
        "lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh",
        "airlock-machine/lifecycle/runtime/tools/scripts/apply-config-candidate.sh",
        "airlock-machine/lifecycle/misc/tools/scripts/nixos-config-vms-smoke.sh",
        "airlock-machine/lifecycle/runtime/tools/scripts/config-boot-verify.sh",
        "lifecycle/misc/tools/scripts/resolve-vm-image-result.sh",
    ):
        subprocess.run(["bash", "-n", str(repo_root / relative)], check=True)


@pytest.fixture
def activation_flow(tmp_path, repo_root):
    scripts = tmp_path / "lifecycle/misc/tools/scripts"
    scripts.mkdir(parents=True)
    for name in ("run-config-boot-activation-flow.sh", "resolve-vm-image-result.sh"):
        shutil.copy(repo_root / "lifecycle/misc/tools/scripts" / name, scripts / name)
    (scripts / "lib").mkdir()
    (scripts / "lib/airlock-env.sh").write_text(
        'airlock_physical_store_path() { printf "%s\\n" "$1"; }\n'
    )
    image = tmp_path / "image"
    image.mkdir()
    logs = tmp_path / "logs"
    logs.mkdir()
    commands = tmp_path / "bin"
    commands.mkdir()
    nix = commands / "nix"
    nix.write_text(
        f"#!{sys.executable}\nimport os\nfrom pathlib import Path\n"
        "with Path(os.environ['FLOW_EVENTS']).open('a') as stream: stream.write('resolve\\n')\n"
        "print(os.environ['FLOW_IMAGE'])\n"
    )
    nix.chmod(0o755)
    runner = scripts / "run-nested-gui-smoke.sh"
    runner.write_text(f"#!{sys.executable}\n" + VM_RUNNER)
    runner.chmod(0o755)
    stage = {
        "schema": "airlock.config-vms-smoke.v1",
        "status": "passed",
        "dom0_system_apply": {
            "status": "passed",
            "host_system_source": "/nix/store/new-host",
            "host_system_switch_action": "boot",
            "reboot_required": True,
        },
        "config_status": {
            "pending_reboot": {"required": True},
            "host_generation": {
                "active": False,
                "staged_source": "/nix/store/new-host",
                "pending_reboot": {"required": True},
            },
        },
    }
    verify = {
        "schema": "airlock.config-boot-verify.v1",
        "status": "passed",
        "expected_host_system_source": "/nix/store/new-host",
        "host_system_profile_resolved_target": "/nix/store/new-host",
        "run_current_system_resolved_target": "/nix/store/new-host",
        "pending_reboot": {"required": False},
        "status_json": {
            "host_generation": {
                "active": True,
                "current_source": "/nix/store/new-host",
                "pending_reboot": {"required": False},
            }
        },
    }
    events = tmp_path / "events"
    calls = tmp_path / "calls.jsonl"

    def run(damage=None):
        if damage == "missing-source":
            stage["dom0_system_apply"].pop("host_system_source")
        elif damage == "not-pending":
            stage["config_status"]["pending_reboot"]["required"] = False
        elif damage == "wrong-running":
            verify["run_current_system_resolved_target"] = "/nix/store/old-host"
        result = subprocess.run(
            ["bash", str(scripts / "run-config-boot-activation-flow.sh")],
            env=dict(
                os.environ,
                PATH=f"{commands}:{os.environ['PATH']}",
                AIRLOCK_LOG_DIR=str(logs),
                AIRLOCK_VM_DIR=str(tmp_path / "vm"),
                FLOW_IMAGE=str(image),
                FLOW_EVENTS=str(events),
                FLOW_CALLS=str(calls),
                FLOW_STAGE=json.dumps(stage),
                FLOW_VERIFY=json.dumps(verify),
            ),
            capture_output=True,
            text=True,
        )
        return (
            result,
            json.loads((logs / "vm-config-boot-activate-flow.json").read_text()),
            [json.loads(line) for line in calls.read_text().splitlines()],
        )

    return run, events, image


VM_RUNNER = r"""
import json, os
from pathlib import Path
phase = 'verify' if os.environ['AIRLOCK_RUN_CONFIG_BOOT_VERIFY'] == '1' else 'stage'
keys = ('AIRLOCK_VM_DIR', 'AIRLOCK_VM_IMAGE_ATTR', 'AIRLOCK_VM_SKIP_BASE_BUILD', 'AIRLOCK_VM_FRESH')
with Path(os.environ['FLOW_CALLS']).open('a') as stream:
    stream.write(json.dumps(dict(phase=phase, **{key: os.environ[key] for key in keys})) + '\n')
with Path(os.environ['FLOW_EVENTS']).open('a') as stream: stream.write(phase + '\n')
name = 'vm-gui-config-boot-verify.json' if phase == 'verify' else 'vm-gui-config-vms-smoke.json'
Path(os.environ['AIRLOCK_LOG_DIR'], name).write_text(os.environ['FLOW_' + phase.upper()])
"""


def test_boot_activation_reuses_one_image_and_verifies_the_staged_system(
    activation_flow,
):
    run, events, image = activation_flow
    result, report, calls = run()
    assert result.returncode == 0, result.stderr
    assert report["status"] == "passed"
    assert report["expected_host_system_source"] == "/nix/store/new-host"
    assert events.read_text().splitlines() == ["resolve", "stage", "verify"]
    assert calls[0]["AIRLOCK_VM_DIR"] == calls[1]["AIRLOCK_VM_DIR"]
    assert calls[0]["AIRLOCK_VM_IMAGE_ATTR"] == calls[1]["AIRLOCK_VM_IMAGE_ATTR"]
    assert [call["AIRLOCK_VM_FRESH"] for call in calls] == ["1", "0"]
    assert all(call["AIRLOCK_VM_SKIP_BASE_BUILD"] == "1" for call in calls)
    assert next(Path(calls[0]["AIRLOCK_VM_DIR"]).glob("*-result")).resolve() == image


@pytest.mark.parametrize(
    "damage,phases",
    [
        ("missing-source", ["stage"]),
        ("not-pending", ["stage"]),
        ("wrong-running", ["stage", "verify"]),
    ],
)
def test_boot_activation_rejects_incomplete_or_wrong_generation(
    activation_flow, damage, phases
):
    result, report, calls = activation_flow[0](damage)
    assert result.returncode != 0
    assert report["status"] == "failed"
    assert [call["phase"] for call in calls] == phases
