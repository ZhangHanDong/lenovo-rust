//! WinMon 1.0 —— 结业集成
//!
//! 把前十四课汇成一条流水线：**采集 → 过滤 → 报告**。
//!   - 采集：`Collector` trait（L14 洋葱架构，Windows/Unix 各一份实现，测试用 Fake）；
//!   - 过滤：按 CPU 阈值筛（L3 借用 / L14 core）；
//!   - 报告：`Vec<Box<dyn Reporter>>` 多格式输出（L5）。
//!
//! 这一层零平台依赖、零 unsafe——真实采集器在 `platform`（见各课 stage 与 SOLUTION）。

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: u8,
}

/// 采集器抽象（L14）。
pub trait Collector {
    fn collect(&self) -> Vec<ProcessInfo>;
}

/// 报告格式（L5）。
pub trait Reporter {
    fn render(&self, procs: &[&ProcessInfo]) -> String;
}

pub struct TextReporter;
pub struct JsonReporter;

impl Reporter for TextReporter {
    fn render(&self, procs: &[&ProcessInfo]) -> String {
        let mut out = String::from("PID\tCPU\tNAME\n");
        for p in procs {
            out.push_str(&format!("{}\t{}%\t{}\n", p.pid, p.cpu, p.name));
        }
        out
    }
}

impl Reporter for JsonReporter {
    fn render(&self, procs: &[&ProcessInfo]) -> String {
        // 注:{:?} 的转义是 Rust 风格(非 ASCII 会输出 \u{...}),不是严格的 JSON 转义。
        // 教学骨架取其简;生产请用 serde_json(结业加分项:替换并加一个非 ASCII 进程名测试)。
        let items: Vec<String> = procs
            .iter()
            .map(|p| format!(r#"{{"pid":{},"cpu":{},"name":{:?}}}"#, p.pid, p.cpu, p.name))
            .collect();
        format!("[{}]", items.join(","))
    }
}

/// 一次分析的汇总。
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub flagged: usize,
    pub by_name: BTreeMap<String, u32>,
}

/// 流水线：采集 → 筛出高 CPU → 交给每个 reporter 出一份报告。
///
/// 返回 (每种格式的报告文本, 汇总统计)。
pub fn run(
    collector: &dyn Collector,
    threshold: u8,
    reporters: &[Box<dyn Reporter>],
) -> (Vec<String>, Summary) {
    todo!("L15：采集 → 借用筛 cpu>=threshold → 每个 reporter 渲染 → 汇总统计")
}

/// 平台无关的 Fake 采集器——让端到端测试在任何平台跑通。
pub struct FakeCollector(pub Vec<ProcessInfo>);
impl Collector for FakeCollector {
    fn collect(&self) -> Vec<ProcessInfo> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake() -> FakeCollector {
        FakeCollector(vec![
            ProcessInfo {
                pid: 4,
                name: "svc".into(),
                cpu: 95,
            },
            ProcessInfo {
                pid: 8,
                name: "idle".into(),
                cpu: 3,
            },
        ])
    }

    #[test]
    fn end_to_end_pipeline() {
        let reporters: Vec<Box<dyn Reporter>> =
            vec![Box::new(TextReporter), Box::new(JsonReporter)];
        let (reports, summary) = run(&fake(), 90, &reporters);

        assert_eq!(reports.len(), 2);
        assert!(reports[0].contains("svc")); // 文本报告含高 CPU 进程
        assert!(reports[1].starts_with('[')); // JSON 报告
        assert!(!reports[1].contains("idle")); // idle 被过滤掉

        assert_eq!(summary.total, 2);
        assert_eq!(summary.flagged, 1);
        assert_eq!(summary.by_name["svc"], 95);
    }
}
