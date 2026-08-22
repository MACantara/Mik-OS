use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let link = Path::new(&manifest).join("link.x");
    println!("cargo:rustc-link-arg=-T{}", link.display());
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=build.rs");
}
