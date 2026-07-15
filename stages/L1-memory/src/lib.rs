//! L1 作业：布局报告
//!
//! 目标：为 WinMon 会用到的类型打印 size / align，并对每一行给一句解释。
//!
//! 下面 `ai_draft` 模块是「AI 生成的第一版」——它能编译，但**有三处断言是错的**。
//! 你的任务：
//!   1. 用你的内存图（L1 的四张图）找出这三处错误；
//!   2. 在 `fixed` 模块里写出正确的版本，让 `cargo test` 全绿；
//!   3. 在 PR 里写明：AI 错在哪、你怎么发现的。

use std::mem::{align_of, size_of};

/// 报告一行：类型名、大小、对齐、一句人话解释。
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRow {
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
    pub note: &'static str,
}

// ───────────────────────── AI 生成的第一版（含 3 处错误）─────────────────────────
// ⚠️ 不要直接信它。先用内存图审查，再跑 tests 验证。
pub mod ai_draft {
    use super::*;

    pub fn report() -> Vec<LayoutRow> {
        vec![
            LayoutRow {
                name: "u8",
                size: 1,
                align: 1,
                note: "一个字节",
            },
            LayoutRow {
                name: "bool",
                size: 1,
                align: 1,
                note: "只有 0/1 两个合法位模式",
            },
            // ↓↓↓ 下面这些里藏着 3 个错误 ↓↓↓
            LayoutRow {
                name: "char",
                size: 1,
                align: 1,
                note: "一个字符",
            }, // ？
            LayoutRow {
                name: "&str",
                size: 8,
                align: 8,
                note: "指向字符串的指针",
            }, // ？
            LayoutRow {
                name: "String",
                size: 24,
                align: 8,
                note: "ptr + cap + len",
            },
            LayoutRow {
                name: "Vec<u32>",
                size: 32,
                align: 8,
                note: "元素越大，Vec 越大",
            }, // ？
            LayoutRow {
                name: "Box<u64>",
                size: 8,
                align: 8,
                note: "瘦指针，指向堆",
            },
            LayoutRow {
                name: "()",
                size: 0,
                align: 1,
                note: "零大小类型",
            },
        ]
    }
}

// ───────────────────────── 你的正确版本 ─────────────────────────
pub mod fixed {
    use super::*;

    /// 参考实现：用 `size_of::<T>()` / `align_of::<T>()` 求出十个类型的布局。
    /// 每一行的 size/align 都不写死数字——这样它永远不会骗你（本课的核心经验）。
    ///
    /// （同步到学员仓库时，哨兵之间的函数体会被挖成 `todo!()`，作为作业起点。）
    pub fn layout_report() -> Vec<LayoutRow> {
        vec![
            LayoutRow {
                name: "u8",
                size: size_of::<u8>(),
                align: align_of::<u8>(),
                note: "一个字节",
            },
            LayoutRow {
                name: "bool",
                size: size_of::<bool>(),
                align: align_of::<bool>(),
                note: "只有 0/1 两个合法位模式",
            },
            // ↓↓↓ 下面这些里藏着 3 个错误 ↓↓↓
            LayoutRow {
                name: "char",
                size: size_of::<char>(),
                align: align_of::<char>(),
                note: "Unicode 标量值，固定 4 字节",
            },
            LayoutRow {
                name: "&str",
                size: size_of::<&str>(),
                align: align_of::<&str>(),
                note: "胖指针：地址 + 长度",
            },
            LayoutRow {
                name: "String",
                size: size_of::<String>(),
                align: align_of::<String>(),
                note: "ptr + cap + len",
            },
            LayoutRow {
                name: "Vec<u32>",
                size: size_of::<Vec<u32>>(),
                align: align_of::<Vec<u32>>(),
                note: "ptr + len + cap，元素存放在堆上",
            },
            LayoutRow {
                name: "&[u8]",
                size: size_of::<&[u8]>(),
                align: align_of::<&[u8]>(),
                note: "切片胖指针：地址 + 长度",
            },
            LayoutRow {
                name: "Vec<u64>",
                size: size_of::<Vec<u64>>(),
                align: align_of::<Vec<u64>>(),
                note: "容器本身仍是 ptr + len + cap",
            },
            LayoutRow {
                name: "Box<u64>",
                size: size_of::<Box<u64>>(),
                align: align_of::<Box<u64>>(),
                note: "瘦指针，指向堆",
            },
            LayoutRow {
                name: "()",
                size: size_of::<()>(),
                align: align_of::<()>(),
                note: "零大小类型",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 正确版本的每一行都必须和编译器算出的真实布局一致。
    #[test]
    fn fixed_report_matches_real_layout() {
        let rows = fixed::layout_report();
        let by = |n: &str| rows.iter().find(|r| r.name == n).unwrap();

        assert_eq!(by("char").size, 4, "char 是 4 字节 Unicode 标量");
        assert_eq!(by("&str").size, 16, "&str 是胖指针 = 2 个机器字");
        assert_eq!(by("String").size, 24);
        assert_eq!(by("Vec<u32>").size, 24, "Vec 永远 3 个机器字，与元素无关");
        assert_eq!(by("Vec<u64>").size, by("Vec<u32>").size);
        assert_eq!(by("&[u8]").size, 16, "切片引用也是胖指针");
        assert_eq!(by("Box<u64>").size, 8, "指向定长类型的指针是瘦指针");
        assert_eq!(by("()").size, 0);
    }

    /// 这个测试证明 AI 的初版确实错了——它和真实布局对不上。
    /// （教学用：让学员亲眼看到"能编译 ≠ 正确"。）
    #[test]
    fn ai_draft_has_wrong_assertions() {
        let rows = ai_draft::report();
        let by = |n: &str| rows.iter().find(|r| r.name == n).unwrap();

        // AI 说 char = 1，实际是 4
        assert_ne!(by("char").size, size_of::<char>());
        // AI 说 &str = 8，实际是 16
        assert_ne!(by("&str").size, size_of::<&str>());
        // AI 说 Vec<u32> = 32，实际是 24
        assert_ne!(by("Vec<u32>").size, size_of::<Vec<u32>>());
    }
}
