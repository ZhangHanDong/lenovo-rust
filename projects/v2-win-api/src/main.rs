//! `proclist`：枚举进程并打印的小工具。
//!
//! 在 Windows 上走真实 ToolHelp32Snapshot；在 macOS / Linux 上走桩示例数据，
//! 以便在交付机（macOS）也能 `cargo run -p v2-win-api` 演示核心逻辑。
//!
//! 用法：
//!   proclist            # 列出全部进程（按 pid 升序）
//!   proclist <substr>   # 仅列出名字含 <substr> 的进程（大小写不敏感）

use v2_win_api::{filter_by_name, format_table, list_processes, sort_by_pid};

fn main() {
    let needle = std::env::args().nth(1);

    let mut procs = list_processes();
    sort_by_pid(&mut procs);

    match needle.as_deref() {
        Some(n) => {
            // 过滤返回的是借用视图；为复用 format_table 这里克隆成拥有的 Vec。
            let filtered: Vec<_> = filter_by_name(&procs, n).into_iter().cloned().collect();
            println!("匹配 \"{n}\" 的进程（共 {}）：", filtered.len());
            print!("{}", format_table(&filtered));
        }
        None => {
            println!("进程列表（共 {}）：", procs.len());
            print!("{}", format_table(&procs));
        }
    }
}
