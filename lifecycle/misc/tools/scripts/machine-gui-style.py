#!/usr/bin/env python3
"""Deploy committed GUI style without selecting an OS or restarting a VM."""

import argparse
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess


ROOT = Path(__file__).resolve().parents[4]
SOURCE = 'lifecycle/misc/modules/sys-gui/desktop/gnome/gui-style.json'


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT / 'airlock-machine'), *args],
                                   text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['plan', 'apply'])
    parser.add_argument('--target', required=True)
    args = parser.parse_args()
    if git('status', '--porcelain'):
        raise RuntimeError('commit the machine source before GUI style deployment')
    revision = git('rev-parse', 'HEAD')
    request = json.loads(git('show', 'HEAD:' + SOURCE))
    if (set(request) != {'schema', 'color_width'}
            or request['schema'] != 'airlock.gui-style.v1'
            or type(request['color_width']) is not int
            or not 1 <= request['color_width'] <= 16):
        raise RuntimeError('invalid committed GUI style')
    request['revision'] = revision
    if args.action == 'plan':
        print(json.dumps({**request, 'action': 'reload-gui-style',
                          'vm_restarts': [], 'host_reboot_required': False,
                          'requires': 'installed GUI border reload support'}))
        return
    spec = importlib.util.spec_from_file_location('machine_deploy',
                                                Path(__file__).with_name('machine-deploy.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    session = module.Dom0(args.target)
    try:
        script = ('printf %s ' + shlex.quote(json.dumps(request) + '\n')
                  + ' | qrexec-client -d sys-gui '
                  + shlex.quote('root:/run/current-system/sw/bin/airlock-gui-style'))
        print(session.run(script))
    finally:
        session.close()


if __name__ == '__main__':
    main()
