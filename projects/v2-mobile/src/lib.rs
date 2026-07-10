//! 第 8 课移动端补充：一个用 **UniFFI（proc-macro 模式）** 导出的平台无关待办（Todo）核心。
//!
//! 角色说明（V2 移动端为**次要平台**，本 crate 复用自 `main` 分支的完整 Android 教学版）：
//! - 本 crate 是「平台无关共享核心 + UniFFI 适配层」的最小可运行样板，配合第 8 课
//!   「跨语言绑定生成」里的移动端小节，演示 Rust ↔ Kotlin/Android 的惯用接入。
//! - **桌面端**：以 `lib`（rlib）形态被 `cargo test` 直接调用，核心逻辑全部可单测。
//! - **Android/Kotlin 端**：以 `cdylib`（.so）形态被加载，UniFFI 生成的 Kotlin 绑定
//!   （`uniffi-bindgen generate`）把下面的类型自动映射为惯用 Kotlin（见本文件末尾「类型映射速查」
//!   与第 8 课讲义）。完整 Android（NDK/JNI/UniFFI/AAR）端到端版见 `main` 分支。
//!
//! 为什么用 proc-macro 模式而非 .udl + build.rs：
//! - 无需额外的 `.udl` 接口文件与 `build.rs` 代码生成步骤，桌面 `cargo build` 风险最低；
//! - 接口「就地」声明在 Rust 类型上，单一事实来源，重构更安全；
//! - 同一套导出宏，`uniffi-bindgen` 也能生成 Swift 绑定（未来 iOS 扩展，见讲义）。
//!
//! 演示的跨界类型映射点（Kotlin 侧形态见文件末尾）：
//! - `Record`（数据类）、`Enum`（密封类/枚举）、`Object`（带方法的有状态对象）；
//! - `Result<T, TodoError>` → Kotlin **受检异常**（`TodoException`）；
//! - `Option<String>` → Kotlin **可空类型**（`String?`）；
//! - `Vec<Todo>` → Kotlin `List<Todo>`。

use std::sync::Mutex;

uniffi::setup_scaffolding!();

/// 待办优先级。`#[derive(uniffi::Enum)]` 会在 Kotlin 侧映射为 `enum class Priority`。
///
/// 🟨 Kotlin 对照：等价于 `enum class Priority { LOW, MEDIUM, HIGH }`。
/// 🟦 C++ 对照：相当于一个 `enum class`，但跨界由 UniFFI 保证编解码一致，无需手写序列化。
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Priority {
    Low,
    Medium,
    High,
}

/// 一条待办记录。`#[derive(uniffi::Record)]` 在 Kotlin 侧映射为 `data class Todo`。
///
/// 映射看点：
/// - `note: Option<String>` → Kotlin `val note: String?`（可空，而非魔法空串）；
/// - `done: bool` → Kotlin `val done: Boolean`；
/// - `id: u64` → Kotlin `val id: ULong`。
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    /// 可选备注：用 `Option` 表达「可能没有」，跨界后是 Kotlin 的可空类型。
    pub note: Option<String>,
    pub priority: Priority,
    pub done: bool,
}

/// 领域错误枚举。`#[derive(uniffi::Error)]` 让 `Result<_, TodoError>` 在 Kotlin 侧
/// 抛出 `TodoException` 的子类（密封异常层级），而不是返回错误码。
///
/// 🟨 Kotlin 对照：调用方写 `try { store.add(...) } catch (e: TodoException.EmptyTitle) { ... }`。
/// 这正是第 2 课「库用 thiserror 定义结构化错误」一路延伸到跨语言边界的收口。
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TodoError {
    /// 标题为空：业务非法输入。
    #[error("标题不能为空")]
    EmptyTitle,
    /// 找不到指定 id 的待办。
    #[error("未找到 id={id} 的待办")]
    NotFound { id: u64 },
}

/// 有状态的待办存储。`#[derive(uniffi::Object)]` 在 Kotlin 侧映射为一个带方法的引用对象，
/// 由 UniFFI 负责跨界对象句柄的生命周期管理（对照手写 JNI 需自己管理的句柄/全局引用）。
///
/// 因为 UniFFI 导出的方法签名是 `&self`（不可变借用），内部用 `Mutex` 做**内部可变性**，
/// 这呼应第 3 课「Arc<Mutex> 跨线程共享」与内部可变性的心智。
#[derive(uniffi::Object)]
pub struct TodoStore {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    items: Vec<Todo>,
}

#[uniffi::export]
impl TodoStore {
    /// 构造一个空存储。`#[uniffi::constructor]` 在 Kotlin 侧生成 `TodoStore()` 构造函数。
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    /// 新增一条待办。
    ///
    /// - `note: Option<String>` → Kotlin 形参 `note: String?`；
    /// - 返回 `Result<Todo, TodoError>` → Kotlin 端「成功返回 `Todo`，失败抛 `TodoException`」。
    ///
    /// 业务规则：标题去除首尾空白后不得为空，否则返回 `TodoError::EmptyTitle`。
    pub fn add(
        &self,
        title: String,
        note: Option<String>,
        priority: Priority,
    ) -> Result<Todo, TodoError> {
        if title.trim().is_empty() {
            return Err(TodoError::EmptyTitle);
        }
        let mut inner = self.inner.lock().expect("TodoStore mutex 不应中毒");
        inner.next_id += 1;
        let todo = Todo {
            id: inner.next_id,
            title: title.trim().to_string(),
            note,
            priority,
            done: false,
        };
        inner.items.push(todo.clone());
        Ok(todo)
    }

    /// 列出全部待办。`Vec<Todo>` → Kotlin `List<Todo>`。
    pub fn list(&self) -> Vec<Todo> {
        self.inner
            .lock()
            .expect("TodoStore mutex 不应中毒")
            .items
            .clone()
    }

    /// 按 id 查找；`Option<Todo>` → Kotlin 可空返回 `Todo?`。
    pub fn find(&self, id: u64) -> Option<Todo> {
        self.inner
            .lock()
            .expect("TodoStore mutex 不应中毒")
            .items
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    /// 把指定待办标记为完成；找不到则返回 `TodoError::NotFound`。
    pub fn complete(&self, id: u64) -> Result<Todo, TodoError> {
        let mut inner = self.inner.lock().expect("TodoStore mutex 不应中毒");
        let todo = inner
            .items
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(TodoError::NotFound { id })?;
        todo.done = true;
        Ok(todo.clone())
    }

    /// 未完成待办数量。`u64` → Kotlin `ULong`。
    pub fn pending_count(&self) -> u64 {
        self.inner
            .lock()
            .expect("TodoStore mutex 不应中毒")
            .items
            .iter()
            .filter(|t| !t.done)
            .count() as u64
    }
}

impl Default for TodoStore {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────── 类型映射速查（Kotlin 侧形态） ─────────────────────────────
//
// Rust（本文件）                         Kotlin（uniffi-bindgen 生成）
// ----------------------------------     -------------------------------------------------
// #[derive(uniffi::Record)] Todo         data class Todo(val id: ULong, val title: String,
//                                                         val note: String?, ...)
// #[derive(uniffi::Enum)]   Priority     enum class Priority { LOW, MEDIUM, HIGH }
// #[derive(uniffi::Error)]  TodoError    sealed class TodoException : Exception()
//                                          ├─ class EmptyTitle
//                                          └─ class NotFound(val id: ULong)
// #[derive(uniffi::Object)] TodoStore    class TodoStore : AutoCloseable { ... }  // close()/use {}
// Option<String>                         String?
// Result<Todo, TodoError>                @Throws(TodoException::class) fun add(...): Todo
// Vec<Todo>                              List<Todo>
// u64                                    ULong
//
// 生成命令（先 `cargo build -p v2-mobile --release` 产出 host dylib；详见第 8 课讲义）：
//   cargo run -p v2-mobile --bin uniffi-bindgen -- generate \
//             --library target/release/libv2_mobile.dylib \
//             --language kotlin --out-dir ./kotlin

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_list_and_option_mapping() {
        let store = TodoStore::new();
        // note 为 None：演示 Option<String> → Kotlin String?
        let t1 = store
            .add("写讲义".to_string(), None, Priority::High)
            .expect("合法标题应成功");
        // note 为 Some：另一条带备注
        let t2 = store
            .add(
                "跑通构建".to_string(),
                Some("cargo build -p v2-mobile".to_string()),
                Priority::Medium,
            )
            .expect("合法标题应成功");

        assert_eq!(t1.id, 1);
        assert_eq!(t2.id, 2);
        assert_eq!(t1.note, None);
        assert_eq!(t2.note.as_deref(), Some("cargo build -p v2-mobile"));

        let all = store.list();
        assert_eq!(all.len(), 2);
        assert_eq!(store.pending_count(), 2);
        assert_eq!(store.find(1), Some(t1));
        assert_eq!(store.find(99), None);
    }

    #[test]
    fn empty_title_maps_to_error() {
        let store = TodoStore::new();
        // 跨界后这是 Kotlin 的 TodoException.EmptyTitle
        let err = store
            .add("   ".to_string(), None, Priority::Low)
            .unwrap_err();
        assert!(matches!(err, TodoError::EmptyTitle));
        assert_eq!(err.to_string(), "标题不能为空");
        assert_eq!(store.list().len(), 0);
    }

    #[test]
    fn complete_updates_state_and_not_found_error() {
        let store = TodoStore::new();
        let t = store
            .add("评审 PR".to_string(), None, Priority::Low)
            .expect("合法标题应成功");
        assert!(!t.done);

        let done = store.complete(t.id).expect("应能完成已存在的待办");
        assert!(done.done);
        assert_eq!(store.pending_count(), 0);

        // 找不到 id → NotFound{ id }，跨界后是 TodoException.NotFound(id)
        let err = store.complete(404).unwrap_err();
        assert!(matches!(err, TodoError::NotFound { id } if id == 404));
        assert_eq!(err.to_string(), "未找到 id=404 的待办");
    }
}
