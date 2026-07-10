//! 纯 Rust 核心逻辑（与 FFI 无关，桌面可单元测试）。

/// 一条待办。
#[derive(Debug, Clone)]
struct Todo {
    id: u32,
    title: String,
    done: bool,
}

/// 待办存储：领域核心，不含任何 FFI 细节。
#[derive(Debug, Default)]
pub struct TodoStore {
    todos: Vec<Todo>,
    next_id: u32,
}

/// 领域错误。
#[derive(Debug, PartialEq, Eq)]
pub enum TodoError {
    NotFound,
}

impl TodoStore {
    pub fn new() -> Self {
        Self {
            todos: Vec::new(),
            next_id: 1,
        }
    }

    /// 新增一条待办，返回其 id。
    pub fn add(&mut self, title: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.todos.push(Todo {
            id,
            title: title.to_string(),
            done: false,
        });
        id
    }

    /// 标记完成。id 不存在时返回 `Err(NotFound)`。
    pub fn complete(&mut self, id: u32) -> Result<(), TodoError> {
        match self.todos.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.done = true;
                Ok(())
            }
            None => Err(TodoError::NotFound),
        }
    }

    /// 未完成的条目数。
    pub fn pending_count(&self) -> u32 {
        self.todos.iter().filter(|t| !t.done).count() as u32
    }

    /// 人类可读的概要字符串：总数、未完成数，以及未完成条目的标题列表。
    pub fn summary(&self) -> String {
        let pending_titles: Vec<&str> = self
            .todos
            .iter()
            .filter(|t| !t.done)
            .map(|t| t.title.as_str())
            .collect();
        format!(
            "{} todos, {} pending: [{}]",
            self.todos.len(),
            self.pending_count(),
            pending_titles.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_assigns_incrementing_ids() {
        let mut s = TodoStore::new();
        assert_eq!(s.add("a"), 1);
        assert_eq!(s.add("b"), 2);
        assert_eq!(s.pending_count(), 2);
    }

    #[test]
    fn complete_marks_done_and_decrements_pending() {
        let mut s = TodoStore::new();
        let id = s.add("write slides");
        assert_eq!(s.pending_count(), 1);
        assert_eq!(s.complete(id), Ok(()));
        assert_eq!(s.pending_count(), 0);
    }

    #[test]
    fn complete_unknown_id_is_not_found() {
        let mut s = TodoStore::new();
        s.add("x");
        assert_eq!(s.complete(999), Err(TodoError::NotFound));
    }

    #[test]
    fn summary_reports_counts() {
        let mut s = TodoStore::new();
        s.add("a");
        let id = s.add("b");
        s.complete(id).unwrap();
        assert_eq!(s.summary(), "2 todos, 1 pending: [a]");
    }
}
