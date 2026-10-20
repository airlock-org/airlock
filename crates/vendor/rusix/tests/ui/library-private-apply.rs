use rusix::interop::Nixpkgs;

fn main() {
    let lib = Nixpkgs::new().library();
    lib.apply("optional", [true.into(), 42_i64.into()]);
}
