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

    // The toolchain identity that goes into the build cache key (ADR-021 D2).
    // Resolved here rather than at run time: it is the rustc that built this
    // emitter, which is the one whose output the cache would be serving, and
    // asking for it once at compile time costs no subprocess per build.
    let version = std::process::Command::new(std::env::var("RUSTC").unwrap_or("rustc".into()))
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=NIKAIA_RUSTC_VERSION={version}");
}
