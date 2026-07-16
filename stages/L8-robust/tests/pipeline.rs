//! 集成测试:完整的"读 → 解析 → 过滤 → 健康检查"链路(任务卡需求 2)。
use winmon_l8_robust::{collect_valid, require_healthy};

#[test]
fn full_pipeline_reads_parses_filters_reports() {
    let raw = "4:boot\n\nbad line\n1200:svc up\nabc:oops\n7:ready";
    let lines: Vec<&str> = raw.lines().collect();

    let result = collect_valid(&lines);
    // 3 条好数据进来了,3 条坏行(空行/无分隔符/坏 pid)被计数
    assert_eq!(result.events.len(), 3);
    assert_eq!(result.skipped, 3);
    assert_eq!(result.events[1].pid, 1200);

    // 健康检查:阈值 3 刚好放行,阈值 2 拒绝
    assert!(require_healthy(&result, 3).is_ok());
    assert!(require_healthy(&result, 2).is_err());
}
