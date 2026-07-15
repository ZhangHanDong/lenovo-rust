//! L14 作业：抽出 winmon-core——洋葱架构
//!
//! 把 WinMon 拆成两层：
//!   - `core`：解析 / 过滤 / 聚合 / 报告。**零平台依赖、零 unsafe、零 `#[cfg]`**（硬指标）；
//!   - `platform`：Windows 用 ToolHelp、Linux/macOS 用 sysinfo——都实现 `core::Collector`。
//!
//! 依赖只能朝内：platform 依赖 core，core 不依赖任何人。这样 core 能在**任何平台**
//! 用一个 Fake collector 跑完全部业务测试（Linux CI 就够）。

/// 平台无关核心。**这个模块里 grep 不到一个 `#[cfg]`、一个 `unsafe`。**
pub mod core {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProcessInfo {
        pub pid: u32,
        pub name: String,
        pub cpu: u8, // 占用百分比 0–100
    }

    /// 采集器抽象——core 只认这个 trait，不认它背后是 Windows 还是 Linux。
    pub trait Collector {
        fn collect(&self) -> Vec<ProcessInfo>;
    }

    /// 业务逻辑：筛出 CPU 超过阈值的进程（引用，不拷贝）。
    pub fn high_cpu(procs: &[ProcessInfo], threshold: u8) -> Vec<&ProcessInfo> {
        todo!("L14 core：借用筛选出 cpu >= threshold 的进程")
    }

    /// 业务逻辑：按进程名聚合 CPU 总和。
    pub fn total_cpu_by_name(procs: &[ProcessInfo]) -> std::collections::BTreeMap<String, u32> {
        todo!("L14 core：把同名进程的 cpu 累加进 BTreeMap")
    }
}

/// 平台适配层：每个平台一份，都实现 `core::Collector`。依赖朝内（依赖 core）。
pub mod platform {
    // 真实实现（需平台依赖，见 SOLUTION.md）：
    //   #[cfg(windows)]        用 CreateToolhelp32Snapshot 枚举进程；
    //   #[cfg(target_os="linux")] / #[cfg(target_os="macos")] 用 sysinfo。
    //
    // 这里给一个平台无关的 Fake，让 core 的业务测试在任何平台都能跑。
    use super::core::{Collector, ProcessInfo};

    pub struct FakeCollector(pub Vec<ProcessInfo>);

    impl Collector for FakeCollector {
        fn collect(&self) -> Vec<ProcessInfo> {
            self.0.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::core::*;
    use super::platform::FakeCollector;

    fn sample() -> Vec<ProcessInfo> {
        vec![
            ProcessInfo {
                pid: 4,
                name: "svc".into(),
                cpu: 95,
            },
            ProcessInfo {
                pid: 8,
                name: "idle".into(),
                cpu: 2,
            },
            ProcessInfo {
                pid: 9,
                name: "svc".into(),
                cpu: 40,
            },
        ]
    }

    #[test]
    fn high_cpu_filters() {
        let procs = sample();
        let hot = high_cpu(&procs, 90);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].pid, 4);
    }

    #[test]
    fn aggregate_by_name() {
        let procs = sample();
        let agg = total_cpu_by_name(&procs);
        assert_eq!(agg["svc"], 135); // 95 + 40
        assert_eq!(agg["idle"], 2);
    }

    /// core 只认 Collector trait——喂一个 Fake 就能在任何平台跑完业务逻辑。
    #[test]
    fn core_works_with_fake_collector() {
        let c = FakeCollector(sample());
        let procs = c.collect();
        assert_eq!(high_cpu(&procs, 90).len(), 1);
    }
}
