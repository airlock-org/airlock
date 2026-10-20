#[path = "../../examples/typed-submodule.rs"]
mod submodule;

#[path = "../../examples/typed-values.rs"]
mod values;

use submodule::{Endpoint, Port};
use values::UserName;

fn main() {
    let _endpoint = Endpoint {
        host: UserName("admin".into()),
        port: Port(443),
    };
}
