use std::{env, fs, io::Write, path::PathBuf};

/// This build script copies the `memory.x` file from the crate root into
/// a directory where the linker can always find it at build time.
/// You may instead place the `memory.x` in the `default-layout.rpk.conf` file.
pub fn build_rs() {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let memoryx = manifest.join("memory.x");
    if !fs::exists(&memoryx).unwrap() {
        panic!("Missing memory.x file!");
    }

    let out = &PathBuf::from(env::var_os("OUT_DIR").expect("missing OUT_DIR"));
    fs::File::create(out.join("memory.x"))
        .and_then(|mut f| f.write_all(fs::read(memoryx)?.as_slice()))
        .expect("can't create memory.x");

    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");

    if env::var_os("CARGO_FEATURE_DEFMT").is_some() {
        println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    }
}
