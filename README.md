# lenovo-rust · Rust 工程化实战培训（V3）· 学员作业仓库

> 作者：张汉东 ｜ 内部培训资料，严禁外传。配套代码以 `MIT OR Apache-2.0` 授权用于教学。

本仓库是课程的**学员作业仓库**。全程围绕一个贯穿项目 **WinMon**（一个跨平台进程/事件监控器）展开，每课一道作业，以 PR 形式提交、按 D1–D8 维度评审。

## 仓库结构

- `stages/LNN-*/` — 每课的代码骨架。`src/lib.rs` 里 `todo!()` 处就是**你要补全的地方**（参考实现在讲师仓库，不随作业下发）。
- `homework/chNN.md` — 每课的作业说明与验收标准；`homework/README.md` 是作业总纲。
- `RUBRIC.md` — 评分维度 **D1–D8**（D8「AI 协作判断力」是本课程与其他 Rust 课最大的不同）。

## 快速开始

```bash
cargo build --workspace          # 骨架能编译（todo!() 可通过）
cargo test  -p winmon-l1-memory  # 跑某一课的验收测试（补全前是红的，这就是作业）

# 质量门禁（提交作业 PR 的硬性要求，补全后应全绿）
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

> `main` 分支是**起点骨架**：`cargo test` 在补全前必然失败——那正是你要完成的作业，不是仓库坏了。

## 作业提交流程（学员必读）

1. **建分支**：从 `main` 切出 `hw/<你的GitHub ID>/ch<课次>`，例如 `hw/alice/ch01`；
2. **做作业**：按 `homework/ch<课次>.md` 补全对应 `stages/LNN-*/` 里的 `todo!()`；
3. **本地过门禁**：`cargo test` 全绿 + `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings`；
4. **开 PR**：目标分支 `main`，标题格式 **`[hw01] 你的姓名或ID`**（课次两位数字）。**PR 描述必须回答 D8 三问**：
   - AI 帮了什么？
   - 你否决了 AI 的哪些建议？为什么？
   - AI 犯了什么错？你怎么发现的？（编译器 / miri / criterion / 你自己）
5. **自动评估**：CI 跑质量门禁（Linux + Windows）；AI 评审按 `RUBRIC.md` 的 D1–D8 维度打分并在 PR 里留评语；
6. **修订**：按 CI 与 AI 评语继续 push 到同一分支，评估自动重跑。

## WinMon 演进线

一行文本(L1) → 所有权建模(L2) → 借用式过滤(L3) → 领域建模(L4) → trait 抽象(L5)
→ 并行采集(L6) → 异步服务(L7) → 容错+性能(L8) → 无分配缓冲(L9) → C 库接入(L10)
→ 真实系统数据(L11) → 服务+通知(L12) → MSI+CI(L13) → 三平台(L14) → 1.0(L15)

> 本仓库随课堂进度滚动更新：每上一课，讲师通过同步脚本补齐对应 `stages/` 与 `homework/`。
