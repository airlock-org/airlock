fn main() {
    let _ = rusix::nix_text!("{outer{inner}}", outer = "value");
}
