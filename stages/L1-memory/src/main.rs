//! 跑一遍布局报告，人眼确认。
use winmon_l1_memory::fixed;

fn main() {
    println!("{:<12} {:>5} {:>6}   说明", "类型", "size", "align");
    println!("{}", "─".repeat(60));
    for row in fixed::layout_report() {
        println!(
            "{:<12} {:>5} {:>6}   {}",
            row.name, row.size, row.align, row.note
        );
    }
}
