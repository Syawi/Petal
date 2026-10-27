fn main() {
    let directory = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let target = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let resource = if target == "gnu" {
        "assets/petal-icon.o"
    } else {
        "assets/petal-icon.res"
    };
    println!(
        "cargo:rustc-link-arg={}",
        directory.join(resource).display()
    );
    println!("cargo:rerun-if-changed={resource}");
}
