fn main() {
    cc::Build::new().file("csrc/checksum.c").compile("checksum");
    println!("cargo:rerun-if-changed=csrc/checksum.c");
}
