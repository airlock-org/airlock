#!/usr/bin/env python3
"""Run the usr-assistant pool-admission apply/rollback flow in nested dom0."""

import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
SSH = ROOT / "lifecycle/misc/tools/scripts/ssh-nested-vm.sh"

DOM0_PROGRAM = r"""
import json
from pathlib import Path
import shlex
import subprocess
import time


def run(*args, input=None, timeout=900):
    return subprocess.run(
        args,
        input=input,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=True,
    ).stdout.strip()


def guest(script, timeout=900):
    args = ["qvm-run", "--pass-io", "--user", "root"]
    if "--no-gui" in run("qvm-run", "--help"):
        args.append("--no-gui")
    return run(*args, "usr-assistant", "bash -s", input=script, timeout=timeout)


def final_json(text):
    decoder = json.JSONDecoder()
    for index, char in enumerate(text):
        if char != "{":
            continue
        try:
            payload, end = decoder.raw_decode(text[index:])
        except json.JSONDecodeError:
            continue
        if not text[index + end:].strip() and isinstance(payload, dict):
            return payload
    raise SystemExit("command did not finish with a JSON object")


if subprocess.run(["qvm-check", "--running", "usr-assistant"]).returncode:
    run("qvm-start", "usr-assistant", timeout=300)
deadline = time.monotonic() + 240
while True:
    try:
        if guest("printf ready", timeout=30) == "ready":
            break
    except subprocess.SubprocessError:
        pass
    if time.monotonic() >= deadline:
        raise SystemExit("usr-assistant qrexec did not become ready")
    time.sleep(3)

topology_path = Path("/var/lib/airlock/system/current-topology.json")
topology = json.loads(topology_path.read_text())
backend = topology["domains"]["usr-assistant"]["backend"]
system = backend["system"]["source"]
root_source = backend["root"]["source"]
kernel_name = backend["kernel"]["name"]
closure_source = "/home/user/.local/share/airlock/pool-admission-smoke/closure"
sentinel = "airlock-pool-admission-private-data"
guest_script = f'''set -eu
mkdir -p {shlex.quote(closure_source)}
printf '%s\\n' {shlex.quote(sentinel)} > /home/user/.airlock-pool-admission-sentinel
nix-store -qR {shlex.quote(system)} > {shlex.quote(closure_source + "/store-paths")}
airlock-pool-send \\
  --domain usr-assistant \\
  --system {shlex.quote(system)} \\
  --closure-source {shlex.quote(closure_source)} \\
  --root-source {shlex.quote(root_source)} \\
  --kernel-name {shlex.quote(kernel_name)} \\
  --source-revision {'1' * 40} \\
  --lock-hash {'2' * 64} \\
  --physical-store-root /nix/store \\
  --nix /run/current-system/sw/bin/nix \\
  --nix-store /
'''
admission_text = guest(guest_script, timeout=3600)
admission = final_json(admission_text)
if admission.get("activated") is not False or admission.get("retained") is not True:
    raise SystemExit(f"unexpected admission result: {admission}")

work = Path("/var/lib/airlock/pool-admission-smoke")
release = work / "release"
release.mkdir(parents=True, exist_ok=True)
updated = json.loads(json.dumps(topology))
updated["domains"]["usr-assistant"]["backend"]["store"]["source"] = closure_source
(release / "topology.json").write_text(json.dumps(updated, sort_keys=True) + "\n")
host = str(Path("/run/current-system").resolve())
setup = str(Path("/run/current-system/sw/bin/airlock-setup-config").resolve())
(release / "release.json").write_text(json.dumps({
    "schema": "airlock.machine-release.v1",
    "profile": "vm-pool-admission-test",
    "host": host,
    "setup": setup,
    "revision": "nested-pool-admission-test",
    "lockHash": "2" * 64,
}, sort_keys=True) + "\n")

apply = final_json(run("airlock-machine-deploy", "apply", str(release), timeout=1800))
after_apply = guest("cat /home/user/.airlock-pool-admission-sentinel", timeout=180)
if after_apply != sentinel:
    raise SystemExit("private sentinel did not survive coordinator apply")
rollback = final_json(run("airlock-machine-deploy", "rollback", timeout=1800))
after_rollback = guest("cat /home/user/.airlock-pool-admission-sentinel", timeout=180)
if after_rollback != sentinel:
    raise SystemExit("private sentinel did not survive coordinator rollback")

print(json.dumps({
    "schema": "airlock.pool-admission-smoke.v1",
    "status": "passed",
    "admission": admission,
    "apply_phase": apply["phase"],
    "rollback_phase": rollback["phase"],
    "sentinel_preserved": True,
}, sort_keys=True))
"""


def main() -> int:
    result = subprocess.run(
        [str(SSH), "sudo", "-n", "python3", "-"],
        input=DOM0_PROGRAM,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=os.environ,
    )
    log_dir = Path(os.environ.get("AIRLOCK_LOG_DIR", ROOT / ".airlock-logs"))
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "pool-admission-smoke.stdout.log").write_text(result.stdout)
    (log_dir / "pool-admission-smoke.stderr.log").write_text(result.stderr)
    if result.returncode:
        sys.stderr.write(result.stderr)
        return result.returncode
    payload = json.loads(result.stdout.splitlines()[-1])
    (log_dir / "pool-admission-smoke.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n"
    )
    print(json.dumps(payload, sort_keys=True))
    subprocess.run(
        [str(SSH), "sudo", "-n", "systemctl", "poweroff"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=os.environ,
        check=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
