# 作业提交与验收方式

每课作业都围绕同一个流程：

```text
需求 → 规格 → 测试 → 实现 → 审查 → PR
```

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
