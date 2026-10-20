
#[rusix::args]
mod views {
    #[rusix(root)]
    struct Inputs {
        #[rusix(expression)]
        value: std::time::Duration,
    }
}

fn main() {}
