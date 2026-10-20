#[path = "../../examples/enum-option.rs"]
mod example;

use example::accepts_connections;

fn main() {
    let _mode = accepts_connections(&"server");
}
