#[path = "../../examples/typed-submodule.rs"]
mod submodule;

#[path = "../../examples/typed-values.rs"]
mod values;

use submodule::{Endpoint, Hostname};
use values::{UserId, listen};

fn main() {
    let _endpoint = Endpoint {
        host: Hostname("service.internal".into()),
        port: UserId(1000),
    };
    listen(UserId(1000));
}
