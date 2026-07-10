//! 构建脚本：用 `cc` crate 把 `csrc/mathlib.c` 编译成静态库并链接进本 crate。
//!
//! `cc` 会自动探测平台默认 C 编译器（macOS / Linux 上是 clang/gcc，
//! Windows MSVC 目标上是 cl.exe），因此这段脚本本身是跨平台的——
//! 对照第 1 课讲的 `build.rs` + 第 11 课的交叉编译，同一份脚本在
//! `cargo-xwin` 交叉编译 MSVC 目标时也无需改动。
//!
//! `rerun-if-changed` 让 C 源/头文件变更时才重新编译，避免每次都重建。

fn main() {
    println!("cargo:rerun-if-changed=csrc/mathlib.c");
    println!("cargo:rerun-if-changed=csrc/mathlib.h");

    cc::Build::new()
        .file("csrc/mathlib.c")
        .include("csrc")
        .warnings(true)
        .compile("mathlib"); // 产出 libmathlib.a 并自动加入链接
}
