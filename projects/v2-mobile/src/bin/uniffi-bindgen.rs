//! UniFFI 绑定生成器入口（in-crate bin）。
//!
//! proc-macro 模式下，绑定从「已编译的动态库」反射元数据生成，因此 bindgen 需与本 crate
//! 同源构建。这里把 uniffi 自带的 CLI 主函数暴露为一个可执行文件：
//!   cargo run -p v2-mobile --bin uniffi-bindgen -- --help
//! 生成 Kotlin 绑定（需先 `cargo build -p v2-mobile --release` 产出 host dylib）：
//!   cargo run -p v2-mobile --bin uniffi-bindgen -- \
//!     generate --library target/release/libv2_mobile.dylib \
//!     --language kotlin --out-dir ./kotlin
fn main() {
    uniffi::uniffi_bindgen_main()
}
