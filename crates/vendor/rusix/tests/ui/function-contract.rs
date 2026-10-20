#[path = "../../examples/function-contracts.rs"]
mod example;

use example::{Endpoint, Hostname, Port, configure_service};

fn main() {
    let endpoint = Endpoint {
        host: Hostname("service.internal".into()),
        port: Port(443),
    };
    let _service = configure_service(endpoint, true);
}
