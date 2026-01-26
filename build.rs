// nikaia/build.rs
fn main() {
    // No build-time generation needed with syn-grammar macros
    println!("cargo:rerun-if-changed=build.rs");
}
