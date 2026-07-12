# 作业提交与验收方式

每课作业都围绕同一个流程：

```text
需求 → 规格 → 测试 → 实现 → 审查 → PR
```

## 作业仓库

所有作业统一在 GitHub 作业仓库中提交和维护：

> **<https://github.com/ZhangHanDong/lenovo-rust>**

该仓库包含课程全部配套 crate（`projects/`）、第 8 课 Flutter 工程（`frb_demo/`）、每课作业说明（`homework/`）与评分维度（`RUBRIC.md`），内容与本书 `projects/` 保持一致。

## 提交流程（PR 制）

1. **建分支**：从 `main` 切出 `hw/<你的 GitHub ID>/ch<课次>`，例如 `hw/alice/ch01`；
2. **做作业**：按 `homework/ch<课次>.md` 的任务说明修改对应 crate（作业位置见下表）；
3. **本地过门禁**：跑通 fmt / clippy / test（命令见下方 PR 模板）；
4. **开 PR**：目标分支 `main`，标题格式 **`[hw01] 你的姓名或 ID`**（课次两位数字），描述里回答本课作业要求的设计问题；
5. **自动评估**：CI 在 **Linux + Windows 双平台**自动跑质量门禁；**AI 评审**自动按 D1–D7 维度逐任务点核对、打分并在 PR 里留结构化评语（总评 / 任务完成度清单 / 评分表 / 必须修改 / 建议改进）；
6. **修订**：按 CI 结果与 AI 评语继续 push 到同一分支，评估自动重跑；
7. **收尾**：作业 PR **不合并**——讲师核定后加 `evaluated` 标签关闭。`main` 始终保持课程基线，下一课作业重新从 `main` 切分支。

> **为什么不合并**：所有学员改的是同一批 crate，合并任何一份都会让 `main` 偏离课程基线。PR 本身（diff + CI 记录 + AI 评语 + 讨论）就是完整的作业档案。
>
> **AI 评分的定位**：AI 给出的 D1–D7 评分（0–3 档：未涉及/初步/熟练/精通，与附录 B 同一量表）供参考与自查，最终成绩由讲师核定；维度定义见「附录 B」。

## 作业放在哪里

作业代码在 `projects/` 或独立演示工程中完成；本节只放作业说明和验收方式。

| 课次 | 作业位置 | 提交重点 |
|---|---|---|
| [第 1 课](./ch01.md) | `projects/v2-idiomatic`、`projects/v2-type-modeling` | 类型建模、newtype、typestate、API 可读性 |
| [第 2 课](./ch02.md) | `projects/v2-ownership-errors` | 所有权边界、错误类型、`thiserror` / `anyhow` 分层 |
| [第 3 课](./ch03.md) | `projects/v2-concurrency` | `Send` / `Sync`、锁粒度、关闭路径、确定性测试 |
| [第 4 课](./ch04.md) | `projects/v2-async` | task、channel、背压、cancel-safety |
| [第 5 课](./ch05.md) | `projects/v2-perf-bench` | criterion、分配优化、性能结论可复现 |
| [第 6 课](./ch06.md) | `projects/v2-safe-abstraction` | unsafe 不变量、`SAFETY` 注释、miri |
| [第 7 课](./ch07.md) | `projects/v2-ffi-cpp` | ABI、字符串、`repr(C)`、跨边界内存所有权 |
| [第 8 课](./ch08.md) | `frb_demo/`、`projects/v2-mobile` | Rust ↔ Flutter 类型映射、生成绑定、对照理解 |
| [第 9 课](./ch09.md) | `projects/v2-win-api` | Win32 调用、feature 裁剪、句柄 RAII |
| [第 10 课](./ch10.md) | `projects/v2-win-com` | COM 公寓、引用计数、WinRT 投影 |
| [第 11 课](./ch11.md) | `projects/v2-win-packaging` | MSVC 目标、cargo-xwin、MSI、签名、CI |
| [第 12 课](./ch12.md) | `projects/v2-win-gui` | WebView2、服务、注册表、事件日志 |
| [第 13 课](./ch13.md) | `projects/v2-cross-core` | 平台无关核心、`cfg`、三平台 CI |
| [第 14 课](./ch14.md) | `projects/v2-debugging` | 报错解读、tracing、回溯、调试记录 |
| [第 15 课](./ch15.md) | `projects/v2-capstone` | 综合交付、FFI、Windows API、CI 与验收报告 |

## PR 描述模板

```markdown
## 变更内容
- 

## 对应规格 / AC
- AC1:
- AC2:

## 验证命令
- [ ] cargo fmt --all --check
- [ ] cargo clippy --workspace --all-targets -- -D warnings
- [ ] cargo test --workspace

## 平台验证
- [ ] macOS / Linux 平台无关部分
- [ ] Windows 实机验证（如适用）

## 风险点
- unsafe / FFI / Windows / async / 性能：
```
