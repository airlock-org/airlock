"""Whole-machine deployment contracts, including interrupted transactions."""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "machine_deploy", ROOT / "lifecycle/runtime/tools/machine-deploy.py"
)
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)


def domain(name, netvm=None, guivm=None):
    return {
        "class": "StandaloneVM",
        "template": None,
        "netvm": netvm,
        "devices": {},
        "prefs": {"autostart": True, "guivm": guivm},
        "features": {},
        "services": {},
        "backend": {
            "type": "nix-closure",
            "state": {"persistent": name + "-private"},
            "system": {"source": "/nix/store/" + name + "-system"},
            "root": {"source": "/nix/store/root/root.img"},
            "kernel": {"name": "qubes-kernel"},
            "store": {
                "source": "/nix/store/" + name + "-closure",
                "mode": "shared-closure",
                "device": "/dev/xvde",
                "blobDevice": "/dev/xvdf",
                "mountpoint": "/nix/store",
                "viewBackend": "erofs-objectmap",
            },
        },
    }


@pytest.fixture
def topology():
    return {
        "schema": "airlock.qubes-topology.v1",
        "kernels": {},
        "prefs": {},
        "domains": {
            "sys-gui": domain("sys-gui"),
            "sys-dev": domain("sys-dev", guivm="sys-gui"),
            "usr-browser": domain("usr-browser", netvm="sys-dev", guivm="sys-gui"),
            "usr-home": domain("usr-home", guivm="sys-gui"),
        },
    }


def test_dependency_order_and_affected_closure(topology):
    new = deepcopy(topology)
    new["domains"]["sys-gui"]["backend"]["system"]["source"] += "-new"
    result = deploy.plan(topology, new, set(topology["domains"]))
    assert result["changed"] == ["sys-gui"]
    assert result["stop"][-1] == "sys-gui"
    assert result["start"][0] == "sys-gui"
    assert result["stop"].index("usr-browser") < result["stop"].index("sys-dev")
    assert set(result["affected"]) == set(topology["domains"])


def test_app_change_does_not_restart_providers(topology):
    new = deepcopy(topology)
    new["domains"]["usr-home"]["prefs"]["vcpus"] = 2
    result = deploy.plan(topology, new, set(topology["domains"]))
    assert result["stop"] == ["usr-home"]
    assert result["stage"] == []


def test_cycle_fails(topology):
    topology["domains"]["sys-gui"]["netvm"] = "sys-dev"
    with pytest.raises(deploy.DeploymentError, match="cycle"):
        deploy.ordered(topology)


@pytest.mark.parametrize(
    "change", ["remove", "class", "devices", "state", "features", "backend"]
)
def test_unsupported_changes_fail(topology, change):
    new = deepcopy(topology)
    vm = new["domains"]["usr-home"]
    if change == "remove":
        del new["domains"]["usr-home"]
    elif change == "state":
        vm["backend"]["state"] = {"persistent": "different"}
    elif change == "features":
        topology["domains"]["usr-home"]["features"]["test"] = "1"
    elif change == "backend":
        vm["backend"]["type"] = "qubes-template"
    else:
        vm[change] = "different"
    with pytest.raises(deploy.DeploymentError):
        deploy.plan(topology, new, set())


def test_dependencies_started_even_without_autostart(topology):
    for vm in topology["domains"].values():
        vm["prefs"]["autostart"] = False
    result = deploy.plan(topology, topology, {"usr-browser"})
    assert result["stop"] == []
    assert result["start"] == ["sys-gui", "sys-dev", "usr-browser"]


def write_release(path, topology, revision="new"):
    path.mkdir()
    deploy.atomic_json(
        path / "release.json",
        {
            "schema": deploy.SCHEMA,
            "profile": "usb-lab",
            "host": "/nix/store/host-" + revision,
            "revision": revision,
            "setup": "/nix/store/setup/bin/airlock-setup-config",
        },
    )
    deploy.atomic_json(path / "topology.json", topology)
    return path


class Machine(deploy.Coordinator):
    def __init__(self, state, baseline, topology):
        self.calls = []
        self.live = set(topology["domains"])
        self.selected = {
            name: vm["backend"]["system"]["source"]
            for name, vm in topology["domains"].items()
        }
        self.initial = baseline
        self.fail = None
        super().__init__(state, self.execute)

    def execute(self, *args, **kwargs):
        self.calls.append(args)
        if self.fail and args[0] == self.fail:
            raise deploy.DeploymentError("simulated failure")
        if args[0] == "qvm-ls":
            return "\n".join(name + "|Running" for name in sorted(self.live))
        if args[0] == "airlock-closure-store":
            assert not any(call[0] == "qvm-shutdown" for call in self.calls)
            return json.dumps({"generation": "staged-generation"})
        if args[0] == "qvm-shutdown":
            self.live.remove(args[-1])
        if args[0] == "qvm-start":
            self.live.add(args[-1])
        if args[0].endswith("/airlock-setup-config"):
            target = deploy.read_json(args[-1])
            self.selected = {
                name: vm["backend"]["system"]["source"]
                for name, vm in target["domains"].items()
            }
            deploy.atomic_json(
                Path(kwargs["env"]["AIRLOCK_LOG_DIR"]) / "setup-config.json",
                {"restart_required": False},
            )
        if args[0] == "qvm-features":
            return self.selected[args[1]]
        if args[0] == "qrexec-client":
            return self.selected[args[2]]
        return ""

    def baseline(self):
        selection = self.pointer()
        return Path(selection["release"]) if selection else self.initial

    def install_boot_hook(self, release, baseline):
        self.calls.append(("boot-hook",))


@pytest.fixture
def machine(tmp_path, topology, monkeypatch):
    monkeypatch.setattr(deploy, "active_profile", lambda: "usb-lab")
    baseline = write_release(tmp_path / "baseline", topology, "old")
    return Machine(tmp_path / "state", baseline, topology)


def new_release(tmp_path, topology):
    new = deepcopy(topology)
    new["domains"]["usr-home"]["backend"]["system"]["source"] += "-new"
    return write_release(tmp_path / "new", new)


def test_guest_success_and_pending_host_are_distinct(machine, tmp_path, topology):
    release = new_release(tmp_path, topology)
    result = machine.apply(release)
    assert result["phase"] == "reboot-required"
    assert result["observed"]["usr-home"]["running"].endswith("-new")
    assert machine.pointer()["release"] == str(release)
    assert deploy.read_json(machine.state / "latest.json") == result
    assert not any("kill" in call[0] for call in machine.calls)


def test_stage_failure_never_stops_or_selects(machine, tmp_path, topology):
    machine.fail = "airlock-closure-store"
    with pytest.raises(deploy.DeploymentError):
        machine.apply(new_release(tmp_path, topology))
    assert machine.pointer() is None
    assert machine.status()["failed_phase"] == "staging"
    assert not any(call[0] == "qvm-shutdown" for call in machine.calls)


def test_failure_requires_recovery_and_resume_preserves_previous(
    machine, tmp_path, topology
):
    machine.fail = "/nix/store/setup/bin/airlock-setup-config"
    release = new_release(tmp_path, topology)
    with pytest.raises(deploy.DeploymentError):
        machine.apply(release)
    assert machine.pointer()["release"] == str(release)
    with pytest.raises(deploy.DeploymentError, match="unfinished"):
        machine.apply(release)
    machine.fail = None
    machine.calls.clear()
    assert machine.apply(release, recover=True)["phase"] == "reboot-required"
    assert "usr-home" in machine.live
    assert machine.status()["previous"] == str(machine.initial)


def test_rollback_reapplies_whole_previous_release(machine, tmp_path, topology):
    machine.apply(new_release(tmp_path, topology))
    machine.calls.clear()
    result = machine.apply(machine.initial, recover=True)
    assert result["observed"]["usr-home"]["running"].endswith("-system")
    assert result["host_selected"] == "/nix/store/host-old"


def test_lock_prevents_two_writers(machine):
    with machine.locked():
        with pytest.raises(deploy.DeploymentError, match="another"):
            with machine.locked():
                pass


def test_profile_mismatch_fails_before_staging(
    machine, tmp_path, topology, monkeypatch
):
    monkeypatch.setattr(deploy, "active_profile", lambda: "usb-gui-lab")
    with pytest.raises(deploy.DeploymentError, match="profile"):
        machine.apply(new_release(tmp_path, topology))
    assert not machine.calls


def test_boot_refuses_interrupted_selection(machine, tmp_path, topology):
    machine.fail = "/nix/store/setup/bin/airlock-setup-config"
    with pytest.raises(deploy.DeploymentError):
        machine.apply(new_release(tmp_path, topology))
    with pytest.raises(deploy.DeploymentError, match="interrupted"):
        machine.boot_setup("/fallback", ["--apply"])


def test_boot_uses_selected_topology_not_image_seed(
    machine, tmp_path, topology, monkeypatch
):
    release = new_release(tmp_path, topology)
    machine.apply(release)
    calls = []
    monkeypatch.setattr(deploy.os, "execv", lambda *args: calls.append(args))
    machine.boot_setup("/image-setup", ["--apply"])
    assert calls[0][1][-2:] == ["--topology", str(release / "topology.json")]


def test_wrong_guest_generation_is_not_success(machine, topology):
    machine.selected["usr-home"] = "/nix/store/wrong-system"
    with pytest.raises(deploy.DeploymentError, match="selected system"):
        machine.verify(topology, machine.live)


def test_rollback_after_vm_addition_keeps_new_vm_halted(machine, tmp_path, topology):
    new = deepcopy(topology)
    new["domains"]["usr-extra"] = domain("usr-extra")
    machine.apply(write_release(tmp_path / "extra", new))
    assert "usr-extra" in machine.live
    machine.calls.clear()
    result = machine.apply(machine.initial, recover=True)
    assert "usr-extra" not in machine.live
    assert result["plan"]["retained_undeclared"] == ["usr-extra"]
    assert all(call[0] != "qvm-remove" for call in machine.calls)


def test_reapplying_same_release_does_not_lose_rollback(machine, tmp_path, topology):
    release = new_release(tmp_path, topology)
    first = machine.apply(release)
    machine.calls.clear()
    second = machine.apply(release)
    assert second["id"] == first["id"]
    assert second["previous"] == str(machine.initial)
    assert not any(call[0] == "qvm-shutdown" for call in machine.calls)


client_spec = importlib.util.spec_from_file_location(
    "deploy_client", ROOT / "lifecycle/misc/tools/scripts/machine-deploy.py"
)
client = importlib.util.module_from_spec(client_spec)
client_spec.loader.exec_module(client)


def test_client_refuses_uncommitted_sources(monkeypatch):
    monkeypatch.setattr(
        client, "command", lambda *args, **kwargs: " M configuration.nix"
    )
    with pytest.raises(RuntimeError, match="commit repository changes"):
        client.build("usb-lab")


def test_low_space_stops_transfer_before_any_write(monkeypatch):
    paths = {"/nix/store/new": {"narSize": 1024**3}}
    monkeypatch.setattr(client, "command", lambda *args: json.dumps(paths))
    calls = []

    class Session:
        def run(self, script, **kwargs):
            calls.append(script)
            return "" if script == client.REMOTE_INVENTORY else str(1024**3)

    with pytest.raises(RuntimeError, match="No VMs or partitions changed"):
        client.transfer("sys-dev-2", "sys-dev", "/nix/store/release", Session())
    assert len(calls) == 2


def test_existing_closure_requires_no_upload(monkeypatch):
    monkeypatch.setattr(
        client,
        "command",
        lambda *args: json.dumps({"/nix/store/present": {"narSize": 100}}),
    )

    class Session:
        def run(self, script, **kwargs):
            assert script == client.REMOTE_INVENTORY
            return "/nix/store/present"

    client.transfer("sys-dev-2", "sys-dev", "/nix/store/present", Session())


@pytest.mark.parametrize(
    "report,error",
    [
        ("262144 4096", "private-volume space"),
        ("4080335 4096", "reached export"),
        ("-1 4096", "invalid sys-dev"),
        ("100 0", "invalid sys-dev"),
    ],
)
def test_bridge_space_check_does_not_require_python(monkeypatch, report, error):
    calls = []

    def command(*args):
        calls.append(args)
        if args[0] == "ssh":
            assert args == ("ssh", "sys-dev", "stat -f -c '%a %S' /home/user")
            return report
        return json.dumps({"/nix/store/new": {"narSize": 1024**3}})

    class Session:
        def run(self, script, **kwargs):
            return "" if script == client.REMOTE_INVENTORY else str(30 * 1024**3)

    def stop_at_export(*args, **kwargs):
        raise RuntimeError("reached export")

    monkeypatch.setattr(client, "command", command)
    monkeypatch.setattr(client.subprocess, "Popen", stop_at_export)
    with pytest.raises(RuntimeError, match=error):
        client.transfer("sys-dev", "sys-dev", "/nix/store/release", Session())
    assert len(calls) == 2


@pytest.mark.parametrize("controller", ["absent", "present", "broken", "failed"])
def test_client_status_before_and_after_first_deployment(
    monkeypatch, tmp_path, capsys, controller
):
    image_engine = tmp_path / "image-engine"
    image_engine.write_text('#!/bin/sh\necho "image-baseline"\n')
    image_engine.chmod(0o755)
    selected = tmp_path / "controller"
    if controller in ("present", "failed"):
        selected.write_text(
            '#!/bin/sh\necho "selected-release"\n'
            + ("exit 1\n" if controller == "failed" else "")
        )
        selected.chmod(0o755)
    elif controller == "broken":
        selected.symlink_to(tmp_path / "missing")

    class Session:
        def __init__(self, target):
            assert target == "sys-dev"

        def run(self, script, **kwargs):
            script = script.replace(
                "/var/lib/airlock/machine/controller", str(selected)
            ).replace(
                "/run/current-system/sw/bin/airlock-machine-deploy", str(image_engine)
            )
            return subprocess.check_output(["bash", "-e", "-c", script], text=True)

        def close(self):
            pass

    monkeypatch.setattr(client, "Dom0", Session)
    result = client.main(["status", "--target", "sys-dev"])
    output = capsys.readouterr().out
    if controller == "absent":
        assert result == 0 and "image-baseline" in output
    elif controller == "present":
        assert result == 0 and "selected-release" in output
    else:
        assert result == 1 and "image-baseline" not in output
