//! WinMon 1.0 CLI —— 用 Fake 采集器演示流水线（真实采集器按平台在 platform 层接入）。
use winmon::{FakeCollector, JsonReporter, ProcessInfo, Reporter, TextReporter};

fn main() {
    let collector = FakeCollector(vec![
        ProcessInfo {
            pid: 4,
            name: "System".into(),
            cpu: 92,
        },
        ProcessInfo {
            pid: 1200,
            name: "winmon".into(),
            cpu: 12,
        },
    ]);
    let reporters: Vec<Box<dyn Reporter>> = vec![Box::new(TextReporter), Box::new(JsonReporter)];
    let (reports, summary) = winmon::run(&collector, 90, &reporters);

    for r in &reports {
        println!("{r}\n---");
    }
    println!(
        "共 {} 个进程，{} 个超阈值。",
        summary.total, summary.flagged
    );
}
