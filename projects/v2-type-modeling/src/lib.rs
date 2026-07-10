//! 第 1 课核心逻辑：用 **类型驱动设计（type-driven design）** 让非法状态不可表达。
//!
//! 本 crate 不是为了"实现一个网络连接"，而是把第 1 课讲的几条类型建模手法落到
//! 可编译、可测试的代码里：
//! - **typestate 模式**：把"连接所处的状态"编码进**类型参数** `Connection<S>`，
//!   于是"在未认证状态调用 `send`"这类误用**根本无法通过编译**——不是运行时
//!   `panic`，而是编译期就不存在那个方法；
//! - **零大小类型（ZST）+ `PhantomData`**：状态标记 `Disconnected` / `Connected`
//!   / `Authenticated` 不占运行时空间，状态信息只活在类型系统里（零成本抽象）；
//! - **sealed trait**：`ConnectionState` 通过私有父 trait 封印，**外部 crate 无法
//!   新增状态**，保证状态集合是封闭、可穷尽推理的；
//! - **关联常量（associated const）**：每个状态用 `const NAME` 携带元信息，演示
//!   "trait 不只有方法，还能有关联项"。
//!
//! 赏析锚点：`bevy` 的 ECS——用**类型**（组件 `Component`）组织数据、用类型签名
//! 让调度器在编译期推断系统的数据访问，是"类型驱动 + 数据导向"的工业级范本。
//!
//! # 合法的生命周期
//!
//! ```
//! use v2_type_modeling::Connection;
//!
//! let mut conn = Connection::new("db.internal:5432")
//!     .connect()
//!     .authenticate("secret-token")
//!     .expect("token 非空，认证成功");
//!
//! conn.send("PING");
//! conn.send("SELECT 1");
//! assert_eq!(conn.sent_count(), 2);
//!
//! // 用完登出、断开，类型随之回退
//! let _disconnected = conn.logout().disconnect();
//! ```
//!
//! # 非法用法：编译期就被拒绝
//!
//! 下面这段**故意写错**——在尚未认证（`Connected`）的连接上调用 `send`。
//! 它被标记为 `compile_fail`，`cargo test` 会验证它**确实编译不过**：
//! `send` 只在 `impl Connection<Authenticated>` 里存在，`Connection<Connected>`
//! 上根本没有这个方法（报错 `E0599: no method named send`）。
//!
//! ```compile_fail
//! use v2_type_modeling::Connection;
//!
//! let mut conn = Connection::new("db.internal:5432").connect();
//! conn.send("PING"); // ❌ 编译失败：Connected 状态没有 send 方法
//! ```
//!
//! 同理，"对已断开的连接再次 `connect`"或"跳过 `connect` 直接 `authenticate`"
//! 也都不可表达——因为对应方法只长在对应状态的 `impl` 块上。

use core::fmt;
use core::marker::PhantomData;

/// 封印模块：父 trait `Sealed` 是私有的，外部 crate 看得见 [`ConnectionState`]，
/// 却无法为自己的类型实现它（因为实现 `ConnectionState` 必须先实现 `Sealed`，
/// 而 `Sealed` 不对外暴露）。这就是 **sealed trait** 模式。
mod sealed {
    pub trait Sealed {}
}

/// 状态标记 trait：约束哪些类型可以充当 [`Connection`] 的状态参数 `S`。
///
/// - 它继承私有的 `sealed::Sealed`，因此**只有本 crate 内的类型**能实现它
///   （孤儿规则之外再加一道"封印"，把状态集合彻底锁死）；
/// - 关联常量 `NAME` 演示 trait 可以携带数据而不止方法——每个状态自带一个
///   人类可读的名字，供日志/诊断使用。
pub trait ConnectionState: sealed::Sealed {
    /// 该状态的可读名称（关联常量，编译期确定，零运行时成本）。
    const NAME: &'static str;
}

/// 状态标记：未连接。**零大小类型（ZST）**——`size_of::<Disconnected>() == 0`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// 状态标记：已建立连接、但尚未认证。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connected;

/// 状态标记：已认证，可收发数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authenticated;

impl sealed::Sealed for Disconnected {}
impl sealed::Sealed for Connected {}
impl sealed::Sealed for Authenticated {}

impl ConnectionState for Disconnected {
    const NAME: &'static str = "disconnected";
}
impl ConnectionState for Connected {
    const NAME: &'static str = "connected";
}
impl ConnectionState for Authenticated {
    const NAME: &'static str = "authenticated";
}

/// 认证失败的错误。即便在 typestate 之下，"运行时校验"（如 token 是否为空）仍可能
/// 失败——这类失败用 [`Result`] 表达，与"编译期就排除的非法转移"分工明确：
/// **能在编译期排除的用类型，必须运行时才知道的用 `Result`。**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// 提供了空 token。
    EmptyToken,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::EmptyToken => write!(f, "authentication token must not be empty"),
        }
    }
}

impl std::error::Error for AuthError {}

/// 一条连接，其**当前状态由类型参数 `S` 编码**（typestate 模式）。
///
/// `S` 是 [`Disconnected`] / [`Connected`] / [`Authenticated`] 之一——它们都是
/// 零大小类型，因此 `Connection<S>` 的运行时布局与不带状态参数时完全一致：
/// 状态信息只存在于类型系统，**零成本**。
///
/// 字段 `sent` 是贯穿整个生命周期的**聚合状态**：认证后每发送一条消息就 +1，
/// 并在 `authenticate` / `logout` 等状态转移中被原样搬运（move）下去——证明
/// typestate 不止能挡非法调用，还能携带真实业务数据穿越状态机。
pub struct Connection<S: ConnectionState> {
    endpoint: String,
    sent: usize,
    /// 仅用于"占位"类型参数 `S`，自身零大小。没有它，编译器会因为 `S` 未被任何
    /// 字段使用而报错（`E0392: type parameter S is never used`）。
    _state: PhantomData<S>,
}

impl Connection<Disconnected> {
    /// 创建一条尚未连接的连接。起点状态是 [`Disconnected`]。
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Connection {
            endpoint: endpoint.into(),
            sent: 0,
            _state: PhantomData,
        }
    }

    /// 建立连接：`Disconnected` → `Connected`。
    ///
    /// 这是一个**消费型**转移：拿走 `self`、返回一个新状态的连接。旧的
    /// `Connection<Disconnected>` 被 move 掉，于是"已 `connect` 过的连接再
    /// `connect` 一次"自然不可表达。
    #[must_use]
    pub fn connect(self) -> Connection<Connected> {
        Connection {
            endpoint: self.endpoint,
            sent: self.sent,
            _state: PhantomData,
        }
    }
}

impl Connection<Connected> {
    /// 认证：`Connected` → `Authenticated`。
    ///
    /// token 是否合法只能在运行时判定，故返回 [`Result`]：
    /// - token 非空 → `Ok(Connection<Authenticated>)`，可以收发数据；
    /// - token 为空 → `Err(AuthError::EmptyToken)`。
    pub fn authenticate(self, token: &str) -> Result<Connection<Authenticated>, AuthError> {
        if token.is_empty() {
            return Err(AuthError::EmptyToken);
        }
        Ok(Connection {
            endpoint: self.endpoint,
            sent: self.sent,
            _state: PhantomData,
        })
    }

    /// 主动断开：`Connected` → `Disconnected`。
    #[must_use]
    pub fn disconnect(self) -> Connection<Disconnected> {
        Connection {
            endpoint: self.endpoint,
            sent: self.sent,
            _state: PhantomData,
        }
    }
}

impl Connection<Authenticated> {
    /// 发送一条消息。**只有 `Authenticated` 状态才有这个方法**——这正是 typestate
    /// 的核心收益：把"必须先认证才能发送"这条业务规则交给类型系统强制执行。
    ///
    /// 返回当前累计发送条数（聚合行为）。
    pub fn send(&mut self, _message: &str) -> usize {
        self.sent += 1;
        self.sent
    }

    /// 当前累计发送的消息条数。
    #[must_use]
    pub fn sent_count(&self) -> usize {
        self.sent
    }

    /// 登出：`Authenticated` → `Connected`。聚合状态 `sent` 被保留下来，
    /// 因此"登出后重新认证再继续发送"时计数能接着累加。
    #[must_use]
    pub fn logout(self) -> Connection<Connected> {
        Connection {
            endpoint: self.endpoint,
            sent: self.sent,
            _state: PhantomData,
        }
    }
}

/// 对**任意状态**都成立的能力，放进泛型 `impl` 块：只要 `S: ConnectionState`
/// 就能调用。这里演示"用关联常量做编译期分发"——`state_name` 不需要任何运行时
/// 分支，名字直接来自类型 `S::NAME`。
impl<S: ConnectionState> Connection<S> {
    /// 连接的目标端点，对所有状态可读。
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// 当前状态的可读名称，取自 [`ConnectionState::NAME`]（编译期常量）。
    #[must_use]
    pub fn state_name(&self) -> &'static str {
        S::NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC1：合法的完整生命周期可用，且 `send` 只在认证后可调用。
    #[test]
    fn full_lifecycle_send_after_auth() {
        let mut conn = Connection::new("db.internal:5432")
            .connect()
            .authenticate("token")
            .expect("non-empty token authenticates");

        let total = conn.send("PING");
        assert_eq!(total, 1);
        assert_eq!(conn.endpoint(), "db.internal:5432");
    }

    /// AC2：聚合行为正确——多次 `send` 累加，`sent_count` 与之一致。
    #[test]
    fn send_count_aggregates() {
        let mut conn = Connection::new("svc:9000")
            .connect()
            .authenticate("t")
            .unwrap();

        conn.send("a");
        conn.send("b");
        conn.send("c");
        assert_eq!(conn.sent_count(), 3);
    }

    /// AC3：运行时校验失败用 `Result` 表达——空 token 认证返回 `Err`。
    #[test]
    fn empty_token_is_rejected() {
        let result = Connection::new("svc:9000").connect().authenticate("");
        assert!(matches!(result, Err(AuthError::EmptyToken)));
    }

    /// AC4：状态可往返，且聚合状态 `sent` 穿越 logout/再认证被保留。
    #[test]
    fn round_trip_preserves_aggregate_state() {
        let mut conn = Connection::new("svc:9000")
            .connect()
            .authenticate("t")
            .unwrap();
        conn.send("first");

        // 登出回到 Connected，再认证回到 Authenticated，计数应延续而非清零。
        let mut conn = conn.logout().authenticate("t").unwrap();
        conn.send("second");
        assert_eq!(conn.sent_count(), 2);
    }

    /// AC5：sealed trait 的关联常量驱动 `state_name`，各状态名称正确。
    #[test]
    fn state_name_reflects_type_parameter() {
        let disconnected = Connection::new("x");
        assert_eq!(disconnected.state_name(), "disconnected");

        let connected = disconnected.connect();
        assert_eq!(connected.state_name(), "connected");

        let authenticated = connected.authenticate("t").unwrap();
        assert_eq!(authenticated.state_name(), "authenticated");
    }

    /// AC6：状态标记是零大小类型（ZST），typestate 不带来运行时体积开销。
    #[test]
    fn state_markers_are_zero_sized() {
        assert_eq!(core::mem::size_of::<Disconnected>(), 0);
        assert_eq!(core::mem::size_of::<Connected>(), 0);
        assert_eq!(core::mem::size_of::<Authenticated>(), 0);
    }
}
