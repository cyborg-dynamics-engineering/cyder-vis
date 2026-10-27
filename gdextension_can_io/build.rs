fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        // Godot patches the embedded PDB path when hot-reloading the DLL. Its
        // temporary name needs five extra bytes (~ prefix and _999 suffix).
        // These redundant relative components reserve room while still locating
        // the same PDB alongside the DLL, without embedding a machine's path.
        println!("cargo::rustc-link-arg-cdylib=/PDBALTPATH:./././%_PDB%");
    }
}
