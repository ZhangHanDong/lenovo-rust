//! L5 作业：多格式报告输出——trait 抽象与 dyn 的用武之地
//!
//! 分析完事件后要输出报告，格式可能是文本 / JSON / HTML，用户还能**同时要多种**。
//!
//! 设计：
//!   - `trait Reporter { fn render(&self, stats: &Stats) -> String; }`
//!   - 三个实现：Text / Json / Html
//!   - 主流程 `Vec<Box<dyn Reporter>>`，遍历输出全部格式
//!
//! 两个审查重点（AI 常在这里翻车）：
//!   1. `Stats` 只是**只读**传给 reporter —— 一个 `&Stats` 就够，别用 `Rc<RefCell<>>`；
//!   2. 想把不同 reporter 装进同一个 `Vec`，必须用 `dyn`（泛型每个类型是不同的单态，装不进一个 Vec）。

/// 分析结果。只读——reporter 不会改它。
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    pub total: usize,
    pub errors: usize,
    pub top_pid: u32,
}

/// 一种报告格式。`&self` 因为 reporter 自身无状态；`&Stats` 因为只读。
pub trait Reporter {
    fn render(&self, stats: &Stats) -> String;
}

pub struct TextReporter;
pub struct JsonReporter;
pub struct HtmlReporter;

impl Reporter for TextReporter {
    fn render(&self, stats: &Stats) -> String {
        todo!("L5：渲染成文本表格（三行 key: value 即可）")
    }
}

impl Reporter for JsonReporter {
    fn render(&self, stats: &Stats) -> String {
        todo!("L5：渲染成 JSON（手写即可，不必引依赖）")
    }
}

impl Reporter for HtmlReporter {
    fn render(&self, stats: &Stats) -> String {
        todo!("L5：渲染成 HTML 片段（一个 <ul> 三个 <li> 即可）")
    }
}

/// 遍历所有 reporter，输出全部格式。
///
/// 这里 **必须** 是 `dyn`：Text / Json / Html 是三个不同类型，
/// 只有 `Box<dyn Reporter>` 能把它们装进同一个 `Vec` 里统一遍历。
/// 注意 `stats` 是 `&Stats` —— 只读借用，不 clone、不 Rc。
pub fn render_all(reporters: &[Box<dyn Reporter>], stats: &Stats) -> Vec<String> {
    reporters.iter().map(|r| r.render(stats)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> Stats {
        Stats {
            total: 42,
            errors: 3,
            top_pid: 1200,
        }
    }

    #[test]
    fn text_format() {
        let out = TextReporter.render(&stats());
        assert!(out.contains("total: 42"));
        assert!(out.contains("top_pid: 1200"));
    }

    #[test]
    fn json_format() {
        let out = JsonReporter.render(&stats());
        assert_eq!(out, r#"{"total":42,"errors":3,"top_pid":1200}"#);
    }

    #[test]
    fn html_format() {
        let out = HtmlReporter.render(&stats());
        assert!(out.starts_with("<ul>"));
        assert!(out.contains("<li>errors: 3</li>"));
    }

    /// dyn 的核心价值：三种不同类型装进同一个 Vec，统一遍历。
    #[test]
    fn render_all_runs_every_format() {
        let s = stats();
        let reporters: Vec<Box<dyn Reporter>> = vec![
            Box::new(TextReporter),
            Box::new(JsonReporter),
            Box::new(HtmlReporter),
        ];
        let out = render_all(&reporters, &s);
        assert_eq!(out.len(), 3);
        // stats 以 &Stats 只读传入——签名保证 reporter 拿不走它;此断言演示调用后仍可用。
        assert_eq!(s.total, 42);
    }
}
