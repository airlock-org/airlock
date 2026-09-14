"""Development shell opt-in boundaries and terminal transport configuration."""

import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import subprocess
import sys
import termios
import time

import pytest

pytestmark = pytest.mark.xdist_group("dev-dom0-terminal")

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture(scope="module")
def terminal_config():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        usb = f.nixosConfigurations.usb-dev;
        host = usb.config;
        lab = host.specialisation.usb-amd-lab.configuration;
        guest = f.nixosConfigurations.airlock-domain-sys-gui-usb-amd-lab.config;
        policy = builtins.head (builtins.filter (p:
          (p.name or "") == "airlock-dev-dom0-terminal-policy"
        ) host.virtualisation.qubes.packages);
        launcher = builtins.head (builtins.filter (p:
          (p.name or "") == "airlock-dom0-terminal"
        ) guest.environment.systemPackages);
        normal = f.nixosConfigurations.vm-gui-minimal.config;
        normalGuest = f.nixosConfigurations.airlock-domain-sys-gui-minimal.config;
        forbidden = (usb.extendModules {{ modules = [ ({{ lib, ... }}: {{
          airlock.image.devMode.enable = lib.mkForce false;
        }}) ]; }}).config;
      in {{
        handler = builtins.readFile host.environment.etc."qubes-rpc/airlock.DevDom0Shell".source;
        policy = builtins.readFile "${{policy}}/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy";
        launcher = builtins.readFile "${{launcher}}/bin/airlock-dom0-terminal";
        labHost = lab.airlock.devDom0Terminal.host.enable;
        guest = guest.airlock.devDom0Terminal.guest.enable;
        hybridGuest = f.nixosConfigurations.airlock-domain-sys-gui-usb.config.airlock.devDom0Terminal.guest.enable;
        normalHost = normal.airlock.devDom0Terminal.host.enable;
        normalHandler = normal.environment.etc ? "qubes-rpc/airlock.DevDom0Shell";
        normalGuest = normalGuest.airlock.devDom0Terminal.guest.enable;
        sudoPassword = lab.security.sudo.wheelNeedsPassword;
        assertionsPass = builtins.all (a: a.assertion) (host.assertions ++ guest.assertions);
        rejected = map (a: a.message) (builtins.filter (a: !a.assertion) forbidden.assertions);
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    report = json.loads(result.stdout)
    client_path = re.search(r"-e (/nix/store/\S+)", report["launcher"])[1]
    report["client"] = subprocess.run(
        [
            "nix",
            "eval",
            "--impure",
            "--raw",
            "--expr",
            f'builtins.readFile "{client_path}"',
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    logs = Path(os.environ["AIRLOCK_LOG_DIR"]) / "dev-dom0-terminal"
    logs.mkdir(parents=True, exist_ok=True)
    (logs / "config.json").write_text(result.stdout)
    return report


@pytest.mark.nix
def test_usb_only_and_sudo_preserved(terminal_config):
    report = terminal_config
    assert report["assertionsPass"]
    assert report["labHost"] and report["guest"] and report["hybridGuest"]
    assert report["sudoPassword"]
    assert not report["normalHost"]
    assert not report["normalHandler"]
    assert not report["normalGuest"]
    assert any(
        "dom0 terminal requires a development image" in m for m in report["rejected"]
    )
    assert "airlock.DevDom0Shell + sys-gui dom0 allow" in report["policy"]
    assert "user=root" not in report["policy"]
    assert "airlock.DevDom0Shell * @anyvm dom0 deny" in report["policy"]
    assert "/bin/id -un" in report["handler"]
    assert "unexpected dom0 execution user" in report["handler"]
    assert "/bin/runuser" not in report["handler"]
    assert "/bin/script --quiet --return --flush" in report["handler"]
    assert "100x30" in report["launcher"]
    assert "/bin/xterm -hold " in report["launcher"]
    assert "exec 2>&1" in report["handler"]


@pytest.mark.nix
@pytest.mark.parametrize(
    "source,argument", [("", ""), ("usr-browser", ""), ("sys-gui", "command")]
)
def test_handler_rejects_wrong_source_or_arguments(terminal_config, source, argument):
    # All must exit before opening the local PTY; no dom0 account is touched.
    result = subprocess.run(
        ["bash", "-c", quiet_logger(terminal_config["handler"])],
        env={
            **os.environ,
            "QREXEC_REMOTE_DOMAIN": source,
            "QREXEC_SERVICE_ARGUMENT": argument,
        },
        capture_output=True,
        text=True,
        timeout=5,
    )
    assert result.returncode == 1, result.stderr
    assert result.stderr == ""
    assert "dom0-terminal: refused:" in result.stdout


def quiet_logger(text):
    # Do not emit fixture messages to the workstation's actual journal.
    return re.sub(r"/nix/store/[^/]+/bin/logger", "true", text)


def replace_transport(client, command):
    text, count = re.subn(
        r"/nix/store/[^/]+/bin/qrexec-client-vm \\\n"
        r"\s*--no-filter-escape-chars-stdout --no-filter-escape-chars-stderr \\\n"
        r"\s*dom0 airlock.DevDom0Shell",
        lambda _: command,
        client,
    )
    assert count == 1
    return text


@pytest.mark.nix
def test_dom0_startup_stderr_is_forwarded(terminal_config, tmp_path):
    stub = tmp_path / "failed-script.py"
    stub.write_text(
        "import sys\nprint('simulated PTY failure', file=sys.stderr)\nsys.exit(1)\n"
    )
    handler = re.sub(
        r"/nix/store/[^/]+/bin/script",
        f"{sys.executable} {stub}",
        quiet_logger(terminal_config["handler"]),
    )
    handler = handler.replace("= airlock", "= user")
    result = subprocess.run(
        ["bash", "-c", handler],
        env={
            **os.environ,
            "QREXEC_REMOTE_DOMAIN": "sys-gui",
            "QREXEC_SERVICE_ARGUMENT": "",
        },
        capture_output=True,
        text=True,
        timeout=5,
    )
    assert result.returncode == 1
    assert "handler reached" in result.stdout
    assert "starting the dom0 user PTY session" in result.stdout
    assert "simulated PTY failure" in result.stdout
    assert result.stderr == ""


@pytest.mark.nix
@pytest.mark.parametrize("failure", [False, True])
def test_interactive_transport_restores_terminal(terminal_config, tmp_path, failure):
    # Replace only qrexec transport with a local PTY server. No root account,
    # Qubes service, GUI or hardware is used in this protocol smoke check.
    script = shutil.which("script")
    stty = shutil.which("stty")
    if not script or not stty:
        pytest.skip("requires util-linux script and stty")
    remote = tmp_path / "remote.sh"
    remote.write_text("stty rows 30 cols 100\nexec bash --noprofile --norc -i\n")
    relay = tmp_path / "relay.py"
    relay.write_text(
        "import os, select, subprocess, sys\n"
        f"p = subprocess.Popen({[script, '--quiet', '--return', '--flush', '--command', f'bash {remote}', '/dev/null']!r}, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)\n"
        "while True:\n"
        "    ready, _, _ = select.select([0, p.stdout.fileno()], [], [])\n"
        "    if p.stdout.fileno() in ready:\n"
        "        data = os.read(p.stdout.fileno(), 65536)\n"
        "        if not data: break\n"
        "        os.write(1, data)\n"
        "    if 0 in ready:\n"
        "        data = os.read(0, 65536)\n"
        "        if not data: break\n"
        "        os.write(p.stdin.fileno(), data)\n"
        "sys.exit(p.wait(timeout=5))\n"
    )
    client = re.sub(r"/nix/store/[^/]+/bin/stty", stty, terminal_config["client"])
    client = replace_transport(client, f"{sys.executable} {relay}")
    if failure:
        # Fail before any remote PTY exists, as in the reported blank window.
        relay.write_text(
            "import sys\nprint('simulated connection failure')\nsys.exit(1)\n"
        )
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    process = subprocess.Popen(
        ["bash", "-c", client], stdin=slave, stdout=slave, stderr=slave
    )
    output = b""

    def receive_until(marker):
        nonlocal output
        deadline = time.monotonic() + 10
        while marker not in output:
            assert time.monotonic() < deadline, output.decode(errors="replace")
            if select.select([master], [], [], 0.2)[0]:
                output += os.read(master, 65536)

    try:
        if failure:
            receive_until(b"exit code 1")
            assert process.wait(timeout=5) == 1
            assert b"simulated connection failure" in output
            assert termios.tcgetattr(slave) == before
            return
        receive_until(b"bash-")
        os.write(
            master, b"test -t 0 && test -t 1 && printf '\\nPTY_OK\\n'; stty size\n"
        )
        receive_until(b"\r\nPTY_OK\r\n")
        receive_until(b"30 100")
        os.write(
            master,
            b"printf '\\n'; read -rs -p 'Password:' token; printf '\\nPROMPT_OK\\n'\n",
        )
        receive_until(b"\r\nPassword:")
        os.write(master, b"dummy-pty-secret\n")
        receive_until(b"\r\nPROMPT_OK\r\n")
        assert b"dummy-pty-secret" not in output
        os.write(master, b"sleep 30\n")
        receive_until(b"sleep 30")
        os.write(master, b"\x03")
        os.write(master, b"printf '\\nINT_OK\\n'; exit\n")
        receive_until(b"\r\nINT_OK\r\n")
        assert process.wait(timeout=5) == 0
        assert termios.tcgetattr(slave) == before
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        os.close(master)
        os.close(slave)
