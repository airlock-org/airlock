use rusix::{Expr, interop::{raw::NixValue, Package, PackageFunction}};

fn main() {
    let _: PackageFunction<Package> = PackageFunction::from_function_attrs([] as [&str; 0], |_| {
        (Vec::<(&str, NixValue)>::new(), Expr::boolean(true))
    });
}
