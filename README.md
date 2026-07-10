# lenovo-rust · Rust 工程化实战培训 · 配套代码与作业仓库

> 作者：张汉东 ｜ 配套代码以 `MIT OR Apache-2.0` 授权用于教学。

本仓库是培训课程的**配套工程 + 学员作业仓库**：

- `projects/` — 15 课配套 crate（`v2-*` 系列，Cargo workspace）
- `frb_demo/` — 第 8 课 Flutter × Rust（flutter_rust_bridge）独立工程
- `homework/` — 15 课作业说明与验收方式
- `RUBRIC.md` — D1–D7 评分维度（作业与结业考核共用）

## 快速开始

```bash
cargo build --workspace
cargo test  --workspace

# 质量门禁（提交作业 PR 的硬性要求）
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 作业提交流程（学员必读）

1. **建分支**：从 `main` 切出 `hw/<你的GitHub ID>/ch<课次>`，例如 `hw/alice/ch01`；
2. **做作业**：按 `homework/ch<课次>.md` 的任务说明修改对应 crate（如第 1 课改 `projects/v2-type-modeling`）；
3. **本地过门禁**：跑通上面四条命令（fmt / clippy / test）；
4. **开 PR**：目标分支 `main`，标题格式 **`[hw01] 你的姓名或ID`**（课次两位数字）。PR 描述必须包含作业要求回答的设计问题（见对应 `homework/chNN.md`）；
5. **自动评估**：CI 自动跑质量门禁（Linux + Windows 双平台）；AI 评审自动按 `RUBRIC.md` 的 D1–D7 维度打分并在 PR 里留评语；
6. **修订**：根据 CI 结果与 AI 评语继续 push 到同一分支，评估会自动重跑；
7. **收尾**：作业 PR **不合并**——讲师确认后加 `evaluated` 标签关闭。`main` 分支始终保持课程原始代码，下一课作业重新从 `main` 切分支。

> 为什么不合并：所有学员改的是同一批 crate，合并任何一份都会让 `main` 偏离课程基线。PR 本身（diff + CI 记录 + AI 评语 + 讨论）就是完整的作业档案。

## 评分

评分维度见 [RUBRIC.md](./RUBRIC.md)。每课作业侧重的维度在对应 `homework/chNN.md` 末尾标明。AI 评审的分数供参考，最终成绩由讲师核定。

## 平台说明

Windows 专属 crate（`v2-win-*`）的代码用 `#[cfg(windows)]` 隔离：macOS/Linux 上 `cargo build --workspace` 可以编过（平台代码被裁剪），运行时行为需在 Windows 上验证——CI 的 Windows job 会替你跑。
