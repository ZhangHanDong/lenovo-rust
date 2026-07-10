//! 第 8 课核心：用 **diplomat** 做"一次定义、多语言生成"的跨语言绑定。
//!
//! 本 crate 的角色不是"实现某个功能"，而是把第 8 课的主张落到可编译代码里：
//! **领域逻辑只写一遍（纯 Rust），对外暴露面用 `#[diplomat::bridge]` 声明式标注，
//! 由 `diplomat-tool` 在另一步生成 C / C++ / Dart / TypeScript / Kotlin / Python 绑定。**
//!
//! 分层：
//! - [`domain`]：纯 Rust 核心（`TodoStore`），不含任何 FFI 细节，桌面可直接 `#[test]`；
//! - [`ffi`]：`#[diplomat::bridge]` 模块——把核心包成 **opaque 类型** `TodoStore`，
//!   演示构造（`Box<Self>`）、方法调用、返回数值（`u32`）、返回字符串（`DiplomatWrite`）、
//!   以及用 **值类型枚举** 承载 `Result` 错误（`TodoFfiError`）。
//!
//! 赏析锚点：diplomat = 声明式 API 标注 + 模板化多语言代码生成
//! （插件式后端，见 <https://github.com/rust-diplomat/diplomat>）。
//!
//! 注意：本 crate 本体（bridge）在 macOS 上即可 `cargo build`；真正"生成绑定"
//! 是独立一步（`diplomat-tool kotlin ...` 等），讲义中说明，不在 crate 构建里跑。

pub mod domain;

#[diplomat::bridge]
pub mod ffi {
    use crate::domain;

    /// 跨界错误：用 diplomat 值类型枚举表达，目标语言会得到一个对应的枚举/常量。
    pub enum TodoFfiError {
        NotFound,
    }

    #[diplomat::opaque]
    pub struct TodoStore(domain::TodoStore);

    impl TodoStore {
        pub fn new() -> Box<TodoStore> {
            Box::new(TodoStore(domain::TodoStore::new()))
        }

        pub fn add(&mut self, title: &str) -> u32 {
            self.0.add(title)
        }

        pub fn complete(&mut self, id: u32) -> Result<(), TodoFfiError> {
            self.0.complete(id).map_err(|e| match e {
                domain::TodoError::NotFound => TodoFfiError::NotFound,
            })
        }

        pub fn pending_count(&self) -> u32 {
            self.0.pending_count()
        }

        pub fn write_summary(&self, out: &mut diplomat_runtime::DiplomatWrite) {
            use std::fmt::Write;
            let _ = write!(out, "{}", self.0.summary());
        }
    }
}
