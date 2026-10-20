// Model evolution: add Peer; both existing consumers must be updated.
enum Mode {
    Server,
    Client,
    Peer,
}

struct FirewallPolicy {
    allowed_ports: Vec<u16>,
}

struct ServicePolicy {
    accepts_connections: bool,
}

fn firewall_policy(mode: &Mode) -> FirewallPolicy {
    match mode {
        Mode::Server => FirewallPolicy {
            allowed_ports: vec![443],
        },
        Mode::Client => FirewallPolicy {
            allowed_ports: vec![],
        },
    }
}

fn service_policy(mode: &Mode) -> ServicePolicy {
    match mode {
        Mode::Server => ServicePolicy {
            accepts_connections: true,
        },
        Mode::Client => ServicePolicy {
            accepts_connections: false,
        },
    }
}

fn main() {}
