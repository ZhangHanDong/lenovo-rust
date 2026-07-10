//! 第 2 课核心逻辑：一个**健壮的配置加载库**，把本课两条主线落到可编译、可测试的代码里。
//!
//! 角色定位：本 crate 站在「**库**」的视角——它**不打印、不 panic、不退出进程**，只把
//! 失败信息编码成**分层的具体错误枚举** [`ConfigError`]，交给调用方（应用层）决定如何展示
//! / 重试 / 降级。这正是「库用 thiserror 暴露具体错误、应用用 anyhow 聚合」分工的库这一侧。
//!
//! 两条主线：
//! - **错误处理最佳实践**：用 `thiserror` 定义分层错误枚举 [`ConfigError`]；底层错误用
//!   `#[from]` + `?` 自动上浮（[`std::io::Error`] / [`serde_json::Error`]），领域错误
//!   （缺字段 / 非法值）用带上下文的结构化变体 `Missing` / `Invalid` 表达。
//! - **所有权进阶 · 零拷贝读路径**：[`normalize_host`] 返回 [`Cow<str>`]——输入已规范则
//!   **借用**（零分配），否则才**克隆并修改**。常见情况不付出分配代价。
//!
//! 赏析锚点：`thiserror` 的 `#[derive(Error)]`——派生宏在编译期为枚举生成 `Display` 与
//! `Error::source`，让「定义一个地道的错误类型」从样板劳动变成几行声明。

use std::borrow::Cow;

use serde::Deserialize;
use thiserror::Error;

/// 配置加载过程中可能出现的**全部失败**，按来源分层。
///
/// 设计要点：
/// - `Io` / `Parse` 是**底层错误**，用 `#[from]` 让 `?` 自动把它们转换上浮，且通过
///   `#[error("...: {0}")]` 保留原始错误信息（`thiserror` 还会自动实现 `source()`，
///   保住错误链）。
/// - `Missing` / `Invalid` 是**领域错误**，携带结构化上下文（哪个 key、哪个字段、为什么），
///   调用方可据此精确定位，而不是只拿到一句模糊的字符串。
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 读取配置来源失败（如文件不存在 / 权限不足）。`#[from] std::io::Error` 使
    /// `std::fs::read_to_string(..)?` 的错误自动转成本变体。
    // 最佳实践：变体已用 #[from] 注册 source，Display 只描述"本层在做什么"、
    // 不回显 {0}，避免上层 anyhow 打印因果链时叶子错误重复出现。
    #[error("读取配置失败")]
    Io(#[from] std::io::Error),

    /// JSON 语法错误。`#[from] serde_json::Error` 使 `serde_json::from_str(..)?`
    /// 的错误自动转成本变体。
    #[error("解析配置 JSON 失败")]
    Parse(#[from] serde_json::Error),

    /// 缺少必填项。`key` 指出是哪一项，便于调用方给出精确提示。
    #[error("缺少必填配置项: `{key}`")]
    Missing { key: String },

    /// 字段存在但取值非法。`field` 指出字段，`reason` 说明为什么不合法。
    #[error("配置项 `{field}` 非法: {reason}")]
    Invalid { field: String, reason: String },
}

/// 校验通过后的**领域配置**——能构造出本类型，就意味着所有不变量都已满足
/// （非法状态不可表达：`port` 一定在合法范围、必填项一定存在）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
}

/// 反序列化用的「原始」配置：字段全是 `Option`，使「缺字段」不在 JSON 解析阶段失败，
/// 而是留到我们自己的校验阶段，好返回**带 key 的** [`ConfigError::Missing`]。
#[derive(Debug, Deserialize)]
struct RawConfig {
    name: Option<String>,
    host: Option<String>,
    port: Option<u64>,
    #[serde(default)]
    max_connections: Option<u64>,
}

/// 归一化主机名，**演示零拷贝读路径**：
/// - 输入已规范（无首尾空白、且全小写）→ 直接 [`Cow::Borrowed`]，**零分配**；
/// - 否则才 [`Cow::Owned`]：裁掉首尾空白并转小写（这一步必须克隆 + 修改）。
///
/// 收益：绝大多数「本就规范」的输入走借用路径，不为它们付出分配代价；只有需要改写时才分配。
/// 这正是 `Cow<'a, B>`「按需克隆」的典型用法——读多写少时的甜点。
#[must_use]
pub fn normalize_host(raw: &str) -> Cow<'_, str> {
    let needs_change = raw.starts_with(char::is_whitespace)
        || raw.ends_with(char::is_whitespace)
        || raw.bytes().any(|b| b.is_ascii_uppercase());

    if needs_change {
        Cow::Owned(raw.trim().to_ascii_lowercase())
    } else {
        Cow::Borrowed(raw)
    }
}

/// 从 JSON 字符串加载并**校验**配置。
///
/// 演示 `#[from]` + `?`：`serde_json::from_str` 的错误经 `?` 自动转为
/// [`ConfigError::Parse`]，无需手写 `map_err`。随后的领域校验返回结构化的
/// `Missing` / `Invalid`。
///
/// # Errors
/// - [`ConfigError::Parse`]：输入不是合法 JSON；
/// - [`ConfigError::Missing`]：缺少 `name` / `host` / `port`；
/// - [`ConfigError::Invalid`]：`port` 越界 / 为 0，`host` 为空，或 `max_connections` 越界。
pub fn load_from_str(input: &str) -> Result<Config, ConfigError> {
    // `?` 在这里做两件事：解析失败时短路返回，并把 serde_json::Error 经 #[from] 转成 ConfigError。
    let raw: RawConfig = serde_json::from_str(input)?;

    let name = raw
        .name
        .ok_or_else(|| ConfigError::Missing { key: "name".into() })?;

    let host_raw = raw
        .host
        .ok_or_else(|| ConfigError::Missing { key: "host".into() })?;
    // 归一化经 Cow：host_raw 本就规范时 normalize_host 走借用路径、零分配。
    // 此处最终需要拥有的 String 存进 Config，故 into_owned()——零拷贝的价值体现在
    // 「只读 / 比较」的调用点（见 normalize_host 文档与测试）。
    let host = normalize_host(&host_raw).into_owned();
    if host.is_empty() {
        return Err(ConfigError::Invalid {
            field: "host".into(),
            reason: "不能为空".into(),
        });
    }

    let port_raw = raw
        .port
        .ok_or_else(|| ConfigError::Missing { key: "port".into() })?;
    let port = u16::try_from(port_raw).map_err(|_| ConfigError::Invalid {
        field: "port".into(),
        reason: format!("必须在 1..=65535 之间, 实际为 {port_raw}"),
    })?;
    if port == 0 {
        return Err(ConfigError::Invalid {
            field: "port".into(),
            reason: "不能为 0".into(),
        });
    }

    let max_raw = raw.max_connections.unwrap_or(100);
    let max_connections = u32::try_from(max_raw).map_err(|_| ConfigError::Invalid {
        field: "max_connections".into(),
        reason: format!("超出 u32 上限: {max_raw}"),
    })?;

    Ok(Config {
        name,
        host,
        port,
        max_connections,
    })
}

/// 从文件加载配置。演示 `#[from] std::io::Error` + `?`：读文件失败时
/// [`std::io::Error`] 经 `?` 自动转为 [`ConfigError::Io`]，解析与校验复用
/// [`load_from_str`]。库只返回错误，**不打印、不退出**。
///
/// # Errors
/// 同 [`load_from_str`]，外加 [`ConfigError::Io`]（读文件失败）。
pub fn load_from_file(path: &std::path::Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path)?; // io::Error --#[from]--> ConfigError::Io
    load_from_str(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_ok_with_defaults() {
        let cfg = load_from_str(r#"{"name":"svc","host":"db.local","port":5432}"#).unwrap();
        assert_eq!(cfg.name, "svc");
        assert_eq!(cfg.host, "db.local");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.max_connections, 100); // 缺省值
    }

    #[test]
    fn missing_required_field() {
        // 缺 port
        let err = load_from_str(r#"{"name":"svc","host":"db.local"}"#).unwrap_err();
        match err {
            ConfigError::Missing { key } => assert_eq!(key, "port"),
            other => panic!("期望 Missing, 实际 {other:?}"),
        }
    }

    #[test]
    fn invalid_field_value() {
        // port 越界（> 65535）
        let err = load_from_str(r#"{"name":"svc","host":"h","port":70000}"#).unwrap_err();
        match err {
            ConfigError::Invalid { field, .. } => assert_eq!(field, "port"),
            other => panic!("期望 Invalid, 实际 {other:?}"),
        }
    }

    #[test]
    fn parse_error_via_from() {
        // 非法 JSON → serde_json::Error 经 #[from] 上浮为 ConfigError::Parse
        let err = load_from_str("{ not json }").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn cow_borrows_when_already_normalized() {
        // 已规范：借用路径，零分配
        let got = normalize_host("db.local");
        assert!(matches!(got, Cow::Borrowed(_)));
        assert_eq!(got, "db.local");
    }

    #[test]
    fn cow_owns_when_modified() {
        // 含大写 + 首尾空白：拥有路径，克隆并改写
        let got = normalize_host("  DB.Local  ");
        assert!(matches!(got, Cow::Owned(_)));
        assert_eq!(got, "db.local");
    }
}
