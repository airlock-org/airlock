
#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        listen_port: i64,
        #[rusix(rename = "listenPort")]
        duplicate: i64,
    }
}

fn main() {}
