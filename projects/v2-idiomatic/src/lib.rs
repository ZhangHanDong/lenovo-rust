//! 第 1 课核心逻辑：用 **idiomatic Rust** 的方式建模一组事件并做查询。
//!
//! 本 crate 不是为了"实现某功能"，而是把第 1 课讲的几条惯用法落到可编译、可测试的代码里：
//! - **newtype** 给标量包一层语义类型（`UserId`），让误用编译不过；
//! - **借用 vs 拥有的 API 取舍**：查询入参借用、聚合结果按需拥有；
//! - **`impl Trait` 返回迭代器**：零成本、不暴露具体类型；
//! - **builder** 组织可选参数，替代"一堆 `Option` 形参"；
//! - **`From`/`Display`/`Error`** 让类型融入标准生态。
//!
//! 赏析锚点：`serde` 的 `#[derive(Deserialize)]`——trait + 派生宏驱动的零成本反序列化。

use std::fmt;

use serde::Deserialize;

/// newtype：用类型区分"用户 id"与普通 `u64`，避免把任意整数当成用户 id 传入。
///
/// 对照 C++：相当于给 `uint64_t` 包一个 `struct UserId { uint64_t v; };`，
/// 但 Rust 的 newtype 是**零成本**的（运行时与 `u64` 等价）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u64);

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "u{}", self.0)
    }
}

/// 事件类别。和类型让"非法类别"无法表达（对照用字符串/魔法数表示类别）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Login,
    Logout,
    Purchase,
    Error,
}

/// 一条事件记录。`#[derive(Deserialize)]` 由 serde 在编译期生成解析代码。
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub user: UserId,
    pub kind: Kind,
    /// Unix 毫秒时间戳。
    pub ts: u64,
    /// 仅 `purchase` 事件携带金额（分）。用 `Option` 表达"可能没有"。
    #[serde(default)]
    pub amount_cents: Option<u64>,
}

/// 查询条件。用 **builder** 组织可选过滤项，避免 `query(kind, user, since, until)`
/// 这种"位置参数堆叠 + 一串 `Option`"的非惯用 API。
#[derive(Debug, Default, Clone)]
pub struct Query {
    kind: Option<Kind>,
    user: Option<UserId>,
    since: Option<u64>,
}

impl Query {
    pub fn new() -> Self {
        Self::default()
    }

    /// 链式 setter 返回 `self`，支持 `Query::new().kind(..).user(..)`。
    #[must_use]
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn user(mut self, user: UserId) -> Self {
        self.user = Some(user);
        self
    }

    #[must_use]
    pub fn since(mut self, ts: u64) -> Self {
        self.since = Some(ts);
        self
    }

    /// 判断单条事件是否命中查询。私有，作为 [`Query::filter`] 的实现细节。
    fn matches(&self, e: &Event) -> bool {
        self.kind.is_none_or(|k| k == e.kind)
            && self.user.is_none_or(|u| u == e.user)
            && self.since.is_none_or(|s| e.ts >= s)
    }

    /// 在一组事件上执行查询，**返回 `impl Iterator`（借用输入，零拷贝、惰性）**。
    ///
    /// 入参 `&'a [Event]` 是借用：查询不夺走调用方的数据所有权。
    /// 返回迭代器而非 `Vec`：是否物化成集合交给调用方决定（要 `Vec` 就 `.collect()`）。
    pub fn filter<'a>(&'a self, events: &'a [Event]) -> impl Iterator<Item = &'a Event> + 'a {
        events.iter().filter(move |e| self.matches(e))
    }
}

/// 聚合统计结果。这里**按需拥有**数据（返回值离开函数，必须自己持有）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub count: usize,
    /// 仅统计 `purchase` 的金额合计（分）。
    pub revenue_cents: u64,
}

/// 对查询命中的事件做聚合。入参是迭代器 `impl Iterator`——
/// 调用方传 `query.filter(&events)` 即可，无需先 `collect`。
pub fn aggregate<'a>(events: impl Iterator<Item = &'a Event>) -> Stats {
    events.fold(Stats::default(), |mut acc, e| {
        acc.count += 1;
        if let Some(amount) = e.amount_cents {
            acc.revenue_cents += amount;
        }
        acc
    })
}

/// 从 JSON Lines 文本解析事件。坏行计入错误数而非中断（容错）。
///
/// 返回 `(事件, 解析失败行数)`：用类型把"部分成功"显式表达出来。
pub fn parse_events(input: &str) -> (Vec<Event>, usize) {
    let mut events = Vec::new();
    let mut skipped = 0;
    for line in input.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Event>(line) {
            Ok(e) => events.push(e),
            Err(_) => skipped += 1,
        }
    }
    (events, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        {"user":1,"kind":"login","ts":100}
        {"user":1,"kind":"purchase","ts":200,"amount_cents":1500}
        {"user":2,"kind":"purchase","ts":300,"amount_cents":900}
        {"user":2,"kind":"logout","ts":400}
    "#;

    #[test]
    fn parses_and_skips() {
        let (events, skipped) = parse_events(SAMPLE);
        assert_eq!(events.len(), 4);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn builder_query_filters() {
        let (events, _) = parse_events(SAMPLE);
        let q = Query::new().kind(Kind::Purchase);
        let hits: Vec<_> = q.filter(&events).collect();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn aggregate_revenue() {
        let (events, _) = parse_events(SAMPLE);
        let stats = aggregate(Query::new().kind(Kind::Purchase).filter(&events));
        assert_eq!(stats.count, 2);
        assert_eq!(stats.revenue_cents, 2400);
    }

    #[test]
    fn newtype_user_filter() {
        let (events, _) = parse_events(SAMPLE);
        let stats = aggregate(Query::new().user(UserId(1)).filter(&events));
        assert_eq!(stats.count, 2); // user 1: login + purchase
    }

    #[test]
    fn malformed_lines_counted() {
        let (events, skipped) =
            parse_events("not json\n{\"user\":9,\"kind\":\"login\",\"ts\":1}\n{oops");
        assert_eq!(events.len(), 1);
        assert_eq!(skipped, 2);
    }
}
