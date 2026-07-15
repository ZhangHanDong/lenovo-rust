# WinMon 作业仓库

这是联想 Rust 工程化课程（**V3**）的**作业提交仓库**。每课一道作业，以 PR 形式提交。

> 版权：课程内容 IP 归张汉东所有，内部培训资料严禁外传。

## 仓库结构

- `stages/LNN-*/` —— 每课的代码骨架（`src/lib.rs` 里 `todo!()` 处是你要补全的地方；参考实现在讲师仓库，不随作业下发）；
- `homework/chNN.md` —— 每课的作业说明与验收标准；
- `RUBRIC.md` —— 评分维度（D1–D8）。

## 怎么做作业

1. 从对应 `stages/LNN-*/` 的骨架开始（起点已挖空）；
2. 补全 `todo!()`，让 `cargo test` 全绿；
3. **用 AI 协作，但你做裁判**——每个 PR 必须回答 D8 三问：
   - AI 帮了什么？
   - 你否决了 AI 的哪些建议？为什么？
   - AI 犯了什么错？你怎么发现的？（编译器 / miri / criterion / 你自己）
4. 提交 PR，CI 跑质量门禁。

## 本地验证

```bash
cargo build --workspace
cargo test -p winmon-l1-memory      # 跑某一课
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```
