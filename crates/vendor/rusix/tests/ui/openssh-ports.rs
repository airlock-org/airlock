#[path = "../../crates/rusix/src/bin/cli/support.rs"]
mod support;

use support::OpenSsh;

fn main() {
    OpenSsh::new().ports(vec!["twenty-two"]);
}
