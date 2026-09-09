//! `std::cli` - the command line.

/// The arguments the program was started with, the program's own name first,
/// exactly as Part III describes it.
pub fn args() -> std::env::Args {
    std::env::args()
}
