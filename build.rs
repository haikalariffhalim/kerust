use std::fs;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("build_info.rs");
    fs::write(&dest, "pub const BUILD_TIME: &str = \"generated\";\n").unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}