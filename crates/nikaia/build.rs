// Binaries that link `rustc_driver` need to find it at run time. Cargo does not
// add the toolchain's lib directory to the rpath, so the linked binary would
// fail to start with "librustc_driver-*.so: cannot open shared object file".
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let sysroot = std::process::Command::new(std::env::var("RUSTC").unwrap_or("rustc".into()))
        .arg("--print=sysroot")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());

    if let Some(sysroot) = sysroot {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}/lib", sysroot.trim());
    }

    // The exact `rustc` cargo is building with. `tests/diagnostics.rs` compiles
    // emitted Rust with it: the crates it links against were built by this one,
    // and a different toolchain on PATH would only report that.
    println!(
        "cargo:rustc-env=NIKAIA_RUSTC={}",
        std::env::var("RUSTC").unwrap_or("rustc".into())
    );
}
