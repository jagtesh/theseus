// Links Sogen's win32k, built by its own CMake. SOGEN_DIR points at the Sogen checkout; the default
// is the sibling layout this integration was developed in.
fn main() {
    let sogen = std::env::var("SOGEN_DIR").unwrap_or_else(|_| "/Users/jagtesh/code/sogen".to_string());
    let artifacts = format!("{sogen}/build/release/artifacts");

    println!("cargo:rustc-link-search=native={artifacts}");
    println!("cargo:rustc-link-lib=dylib=sogen-win32k");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{artifacts}");
    println!("cargo:rustc-env=SOGEN_ROOT={sogen}/emu/standalone-root");
    println!("cargo:rerun-if-env-changed=SOGEN_DIR");
}
