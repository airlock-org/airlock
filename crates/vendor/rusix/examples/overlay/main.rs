//! Print an overlay customizing nixpkgs' existing curl recipe and a lookup of its build-recipe path.
mod authoring;

mod inputs;

mod model;

fn main() {
    // Describe importing the pinned nixpkgs package collection with our overlay.
    // Curl comes from that collection; the overlay customizes its existing recipe.
    // Nix performs the import later, when the generated expression is evaluated.
    let pkgs = authoring::package_set().view::<inputs::Packages>();

    // Select the customized curl entry, then give its metadata named Rust accessors.
    // PackageMetadata describes fields to read; it does not provide a curl implementation.
    let curl_metadata = pkgs.curl().view::<inputs::PackageMetadata>();

    // Export the overlay function and the customized curl build-recipe path.
    // Nix can compute that recipe path without building curl.
    let output = model::Output {
        overlay: authoring::overlay(),
        curl_derivation: curl_metadata.drv_path(),
    };

    // Generate and print Nix source. Nix evaluation is a separate step.
    let artifact = rusix::compile(output).expect("the overlay has valid structural values");
    println!("{}", artifact.source);
}
