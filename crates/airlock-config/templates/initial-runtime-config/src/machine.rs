//! Declare the initial editable runtime candidate with explicit VM connections and resources.
//! The physical machine has its own hardware profile and resource allocations.

use crate::model::{AuthoringError, GiB, Machine, MachineSnapshot, Memory, MiB, Vm};

/// Build the six-VM controller candidate while preserving its established nested-host resource envelope.
pub fn machine() -> Result<MachineSnapshot, AuthoringError> {
    let mut machine = Machine::default();

    let network = Vm::network("sys-net", GiB(2))?;
    network.set_memory(Memory::Ballooned {
        initial: MiB(256),
        maximum: MiB(512),
    });
    machine.add(&network)?;

    let firewall = Vm::firewall("sys-firewall", &network)?;
    firewall.set_memory(Memory::Ballooned {
        initial: MiB(256),
        maximum: MiB(512),
    });
    machine.add(&firewall)?;

    let home = Vm::personal("usr-home", &firewall, GiB(16))?;
    machine.add(&home)?;

    let gui = Vm::gui("sys-gui", &home)?;
    machine.add(&gui)?;

    let controller = Vm::controller("sys-ctrl", &firewall, GiB(8))?;
    controller.set_root_image(MiB(128));
    controller.set_autostart(false);
    machine.add(&controller)?;

    let command = Vm::command("usr-cmd", &firewall, GiB(8))?;
    machine.add(&command)?;

    machine.set_host(&gui, &firewall, &controller);
    machine.finish()
}
