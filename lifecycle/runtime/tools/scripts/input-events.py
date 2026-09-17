#!/usr/bin/env python3
"""Connect dom0 input forwarding when the selected Qubes GUI domain starts."""

import argparse
import asyncio
import signal
import subprocess


class InputCoordinator:
    """Translate Qubes lifecycle events into local systemd unit activation."""

    def __init__(self, app, target, connect_unit, input_target, systemctl):
        self.app = app
        self.target = target
        self.connect_unit = connect_unit
        self.input_target = input_target
        self.systemctl = systemctl

    def _systemctl(self, action, *units):
        subprocess.run(
            [self.systemctl, "--no-block", action, *units],
            check=True,
        )

    def connect_if_running(self):
        try:
            running = self.app.domains[self.target].is_running()
        except KeyError:
            running = False
        if running:
            self._systemctl("start", self.connect_unit)

    def handle_event(self, subject, event, **_kwargs):
        if event == "connection-established":
            self.connect_if_running()
            return
        if subject is None or subject.name != self.target:
            return
        if event == "domain-start":
            self._systemctl("start", self.connect_unit)
        elif event in {
            "domain-shutdown",
            "domain-stopped",
            "domain-start-failed",
        }:
            self._systemctl("stop", self.connect_unit, self.input_target)


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--target", required=True)
    result.add_argument("--connect-unit", required=True)
    result.add_argument("--input-target", required=True)
    result.add_argument("--systemctl", default="systemctl")
    return result


async def listen(args):
    import qubesadmin.app
    import qubesadmin.events

    app = qubesadmin.app.QubesLocal()
    coordinator = InputCoordinator(
        app,
        args.target,
        args.connect_unit,
        args.input_target,
        args.systemctl,
    )
    events = qubesadmin.events.EventsDispatcher(app)
    for event in (
        "connection-established",
        "domain-start",
        "domain-shutdown",
        "domain-stopped",
        "domain-start-failed",
    ):
        events.add_handler(event, coordinator.handle_event)

    # Cover a service start after the domain is already running. A duplicate
    # domain-start event is harmless because systemd coalesces the start job.
    coordinator.connect_if_running()
    listener = asyncio.create_task(events.listen_for_events())
    loop = asyncio.get_running_loop()
    for signame in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(signame, listener.cancel)
    try:
        await listener
    except asyncio.CancelledError:
        pass


def main():
    asyncio.run(listen(parser().parse_args()))


if __name__ == "__main__":
    main()
