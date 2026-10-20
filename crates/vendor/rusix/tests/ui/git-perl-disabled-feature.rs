#[path = "../../examples/git-nixpkg/model.rs"]
mod model;

fn main() {
    let _ = model::Perl::Disabled { send_email: true };
}
