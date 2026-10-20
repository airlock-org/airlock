#[path = "../../examples/invalid-states.rs"]
mod example;

use example::{Certificate, Transport};

fn main() {
    let _transport = Transport::Plain {
        certificate: Certificate("/run/keys/service.pem".into()),
    };
}
