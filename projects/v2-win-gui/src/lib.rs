//! 第 12 课配套工程：Windows 桌面 GUI 与系统集成。
//!
//! 延续第 9–13 课的跨平台纪律，本 crate 切成「平台无关核心 + `#[cfg(windows)]` 适配层」：
//!
//! - **平台无关核心**（本文件顶层）：托盘菜单数据模型 [`TrayMenu`]/[`MenuItem`]、
//!   应用配置 [`AppConfig`]、以及与 SCM/systemd/WorkManager 同构的服务状态机
//!   [`ServiceState`]/[`ServiceCommand`]。这些都是**纯逻辑**，不碰任何系统 API，
//!   在 macOS / Linux / Windows 上行为一致、可用同一套 `#[test]` 验证。
//! - **Windows 服务适配层**（[`mod@service`]，`#[cfg(windows)]`）：用 `windows-service`
//!   把进程注册进 SCM，用核心状态机驱动对外汇报的服务状态。
//! - **系统集成适配层**（[`mod@integration`]，`#[cfg(windows)]`）：用 `windows` crate
//!   读写注册表（开机自启）、写 Windows 事件日志。
//! - **GUI 适配层**（`mod gui`，`#[cfg(all(windows, feature = "gui"))]`）：用 `tao` 建窗口/
//!   事件循环、`wry` 渲染 WebView2，把核心的 [`TrayMenu`] 渲染成 HTML。GUI 依赖较重，
//!   故以 `gui` feature 默认关闭，避免拖慢 CI。
//!
//! 关键工程取舍：`windows-service` / `windows` / `wry` / `tao` 全部写在本 crate 自己
//! `Cargo.toml` 的 `[target.'cfg(windows)'.dependencies]` 下，**交付机（macOS）根本不会
//! 编译它们**。Windows 代码隔离在 `cfg(windows)` 之后，不拖累其它平台的构建，由客户在
//! Windows 上验证真实路径——这正是 Windows 模块（第 9–15 课）共用的交付契约。
//!
//! 赏析锚点：`wry`/`tao`（Tauri 生态）——跨平台 WebView 抽象如何把各平台的原生 WebView
//! 收敛到统一 API（Windows 落到 WebView2 / Edge Chromium，macOS 落到 WKWebView，
//! Linux 落到 WebKitGTK），与 `serde`/`windows-rs` 同属「统一 trait/抽象 + 各平台后端」主线。
//! <https://github.com/tauri-apps/wry> / <https://github.com/tauri-apps/tao>

#![cfg_attr(docsrs, feature(doc_cfg))]

// ===========================================================================
// 平台无关核心 1/3：服务状态机（与 SCM / systemd / WorkManager 同构）
// ===========================================================================

/// 服务生命周期状态。刻意做成**与具体平台无关的纯枚举**：
///
/// - Windows 上，适配层把它映射到 SCM 的 `SERVICE_RUNNING`/`SERVICE_PAUSED`/`SERVICE_STOPPED`；
/// - Linux 上，对应 systemd 单元的 `active (running)` / `inactive (dead)`；
/// - Android 上，对应 WorkManager/`Service` 的运行—暂停—停止托管。
///
/// 把「合法的状态迁移」收进类型系统（见 [`ServiceState::apply`]），就能在桌面上
/// 用单测穷尽验证生命周期，而不必起一个真服务。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceState {
    /// 未运行（初始态）。
    #[default]
    Stopped,
    /// 正在运行。
    Running,
    /// 已暂停（仍占有资源，可 `Continue` 恢复）。
    Paused,
}

/// SCM 会下发给服务的控制命令（对应 `ServiceControl::Stop/Pause/Continue` 等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCommand {
    /// 启动：仅在 `Stopped` 合法。
    Start,
    /// 停止：在 `Running` / `Paused` 合法。
    Stop,
    /// 暂停：仅在 `Running` 合法。
    Pause,
    /// 恢复：仅在 `Paused` 合法。
    Continue,
}

/// 非法状态迁移。实现 [`std::error::Error`]，因此能用 `?` 传播、`{}` 直接打印。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    /// 迁移前的状态。
    pub from: ServiceState,
    /// 触发迁移的命令。
    pub command: ServiceCommand,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "非法服务迁移：状态 {:?} 不接受命令 {:?}",
            self.from, self.command
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl ServiceState {
    /// 施加一条控制命令，返回新状态或 [`InvalidTransition`]。
    ///
    /// 这是整套生命周期的**唯一真相源**：服务适配层向 SCM 汇报状态前先走它，
    /// 保证「对外汇报的状态」永远是一次合法迁移的结果。
    ///
    /// # Errors
    ///
    /// 当 `command` 在当前状态下非法（如对 `Stopped` 服务 `Pause`）时返回错误。
    pub fn apply(self, command: ServiceCommand) -> Result<Self, InvalidTransition> {
        use ServiceCommand::{Continue, Pause, Start, Stop};
        use ServiceState::{Paused, Running, Stopped};

        let next = match (self, command) {
            (Stopped, Start) => Running,
            (Running, Stop) | (Paused, Stop) => Stopped,
            (Running, Pause) => Paused,
            (Paused, Continue) => Running,
            _ => {
                return Err(InvalidTransition {
                    from: self,
                    command,
                })
            }
        };
        Ok(next)
    }

    /// 是否处于「活动」状态（运行或暂停，均占有资源）。
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, ServiceState::Running | ServiceState::Paused)
    }
}

// ===========================================================================
// 平台无关核心 2/3：托盘 / 窗口菜单数据模型
// ===========================================================================

/// 一个菜单项。GUI 适配层（tao 原生菜单 / wry 网页菜单 / 系统托盘）只负责
/// **渲染**这份纯数据，不反向把平台细节塞进核心。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    /// 普通动作项：点击触发由 `id` 标识的命令。
    Action {
        /// 稳定的命令标识（适配层据此分发点击事件）。
        id: String,
        /// 显示文本。
        label: String,
        /// 是否可点击（置灰时为 `false`）。
        enabled: bool,
    },
    /// 勾选项：表达一个布尔开关（如「开机自启」）。
    Checkbox {
        /// 稳定的命令标识。
        id: String,
        /// 显示文本。
        label: String,
        /// 当前勾选状态。
        checked: bool,
    },
    /// 分隔线。
    Separator,
}

impl MenuItem {
    /// 返回该项的 `id`（分隔线没有 `id`，返回 `None`）。
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            MenuItem::Action { id, .. } | MenuItem::Checkbox { id, .. } => Some(id),
            MenuItem::Separator => None,
        }
    }
}

/// 托盘/窗口菜单：一串 [`MenuItem`] 的有序集合，提供 builder 构造与按 `id` 查改。
///
/// 这是「应用菜单」的**单一数据模型**：原生托盘、原生菜单栏、WebView 内的 HTML 菜单
/// 都从同一个 `TrayMenu` 渲染，保证三种 UI 形态语义一致。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrayMenu {
    items: Vec<MenuItem>,
}

impl TrayMenu {
    /// 空菜单。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个动作项（builder 风格，消费并返回 `self`）。
    #[must_use]
    pub fn action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(MenuItem::Action {
            id: id.into(),
            label: label.into(),
            enabled: true,
        });
        self
    }

    /// 追加一个勾选项。
    #[must_use]
    pub fn checkbox(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        checked: bool,
    ) -> Self {
        self.items.push(MenuItem::Checkbox {
            id: id.into(),
            label: label.into(),
            checked,
        });
        self
    }

    /// 追加一条分隔线。
    #[must_use]
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::Separator);
        self
    }

    /// 只读访问全部菜单项。
    #[must_use]
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// 菜单项数量（含分隔线）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空菜单。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 按 `id` 查找菜单项（借用返回，零拷贝）。
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&MenuItem> {
        self.items.iter().find(|item| item.id() == Some(id))
    }

    /// 切换某个勾选项的状态，返回切换后的新值；目标不存在或非勾选项时返回 `None`。
    pub fn toggle(&mut self, id: &str) -> Option<bool> {
        for item in &mut self.items {
            if let MenuItem::Checkbox {
                id: item_id,
                checked,
                ..
            } = item
            {
                if item_id == id {
                    *checked = !*checked;
                    return Some(*checked);
                }
            }
        }
        None
    }

    /// 设置某个动作项的可用性，成功返回 `true`。
    pub fn set_enabled(&mut self, id: &str, value: bool) -> bool {
        for item in &mut self.items {
            if let MenuItem::Action {
                id: item_id,
                enabled,
                ..
            } = item
            {
                if item_id == id {
                    *enabled = value;
                    return true;
                }
            }
        }
        false
    }

    /// 把菜单渲染成一段极简 HTML（供 wry WebView 显示）。
    ///
    /// 放在核心里是有意为之：**渲染逻辑与平台无关、可在桌面单测**，
    /// GUI 适配层只负责把这段 HTML 喂给 WebView，不掺业务。
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut body = String::from("<ul class=\"menu\">");
        for item in &self.items {
            match item {
                MenuItem::Action { id, label, enabled } => {
                    let cls = if *enabled {
                        "action"
                    } else {
                        "action disabled"
                    };
                    body.push_str(&format!(
                        "<li class=\"{cls}\" data-id=\"{id}\">{label}</li>"
                    ));
                }
                MenuItem::Checkbox { id, label, checked } => {
                    let mark = if *checked { "[x]" } else { "[ ]" };
                    body.push_str(&format!(
                        "<li class=\"checkbox\" data-id=\"{id}\">{mark} {label}</li>"
                    ));
                }
                MenuItem::Separator => body.push_str("<li class=\"sep\"><hr></li>"),
            }
        }
        body.push_str("</ul>");
        body
    }
}

// ===========================================================================
// 平台无关核心 3/3：应用配置 + 系统集成所需的纯数据
// ===========================================================================

/// 界面主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// 跟随系统。
    #[default]
    System,
    /// 亮色。
    Light,
    /// 暗色。
    Dark,
}

/// 应用配置（纯数据 + 校验）。系统集成层会从中取出「开机自启」的注册表写入参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// 应用名（同时用作注册表自启项的值名、事件日志的来源名）。
    pub app_name: String,
    /// 是否开机自启（适配层据此写/删 HKCU `...\Run` 项）。
    pub autostart: bool,
    /// 启动时是否最小化到托盘。
    pub start_minimized: bool,
    /// 界面主题。
    pub theme: Theme,
}

/// 配置校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// 应用名为空。
    EmptyName,
    /// 应用名含注册表/路径不安全字符。
    InvalidNameChar(char),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::EmptyName => write!(f, "应用名不能为空"),
            ConfigError::InvalidNameChar(c) => write!(f, "应用名含非法字符：{c:?}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// HKCU 下「当前用户开机自启」项的子键路径——系统集成层写入注册表时使用。
///
/// 放在核心里作为常量，使「写哪个键」这件事可被审查、被测试，而不必真碰注册表。
pub const AUTOSTART_REG_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

impl AppConfig {
    /// 用应用名构造默认配置。
    #[must_use]
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            autostart: false,
            start_minimized: true,
            theme: Theme::System,
        }
    }

    /// 校验配置是否可安全用于注册表/事件日志。
    ///
    /// # Errors
    ///
    /// 应用名为空或含 `\\ / : * ? " < > |` 等非法字符时返回 [`ConfigError`]。
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.app_name.trim().is_empty() {
            return Err(ConfigError::EmptyName);
        }
        if let Some(c) = self
            .app_name
            .chars()
            .find(|c| matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            return Err(ConfigError::InvalidNameChar(c));
        }
        Ok(())
    }

    /// 注册表自启项的**值名**（即应用名）。供集成层 `RegSetValueExW` 使用。
    #[must_use]
    pub fn autostart_value_name(&self) -> &str {
        &self.app_name
    }
}

// ===========================================================================
// Windows 适配层：仅在 Windows 目标参与编译（依赖见 Cargo.toml 的 cfg(windows) 段）
// ===========================================================================

/// Windows 服务骨架（SCM 注册 + 生命周期）。仅 `#[cfg(windows)]` 编译。
#[cfg(windows)]
pub mod service;

/// 系统集成：注册表（开机自启）+ 事件日志。仅 `#[cfg(windows)]` 编译。
#[cfg(windows)]
pub mod integration;

/// 桌面 GUI：tao 窗口 + wry WebView 骨架。仅 `#[cfg(all(windows, feature = "gui"))]` 编译。
#[cfg(all(windows, feature = "gui"))]
pub mod gui;

// ===========================================================================
// 测试：全部平台无关，可在 macOS 桌面 `cargo test` 跑通
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_state_machine_happy_path() {
        // Stopped -> Running -> Paused -> Running -> Stopped 全程合法。
        let s = ServiceState::default();
        assert_eq!(s, ServiceState::Stopped);
        let s = s.apply(ServiceCommand::Start).unwrap();
        assert_eq!(s, ServiceState::Running);
        assert!(s.is_active());
        let s = s.apply(ServiceCommand::Pause).unwrap();
        assert_eq!(s, ServiceState::Paused);
        assert!(s.is_active());
        let s = s.apply(ServiceCommand::Continue).unwrap();
        assert_eq!(s, ServiceState::Running);
        let s = s.apply(ServiceCommand::Stop).unwrap();
        assert_eq!(s, ServiceState::Stopped);
        assert!(!s.is_active());
    }

    #[test]
    fn service_state_machine_rejects_invalid() {
        // 对 Stopped 服务 Pause / Continue / Stop 都非法。
        let s = ServiceState::Stopped;
        for cmd in [
            ServiceCommand::Pause,
            ServiceCommand::Continue,
            ServiceCommand::Stop,
        ] {
            let err = s.apply(cmd).unwrap_err();
            assert_eq!(err.from, ServiceState::Stopped);
            assert_eq!(err.command, cmd);
        }
        // 重复 Start 也非法（Running 不接受 Start）。
        let running = ServiceState::Running;
        assert!(running.apply(ServiceCommand::Start).is_err());
    }

    #[test]
    fn tray_menu_builder_and_find() {
        let menu = TrayMenu::new()
            .action("open", "打开主界面")
            .checkbox("autostart", "开机自启", false)
            .separator()
            .action("quit", "退出");
        assert_eq!(menu.len(), 4);
        assert!(!menu.is_empty());
        // 分隔线没有 id，不会被 find 命中。
        assert!(menu.find("nope").is_none());
        match menu.find("open") {
            Some(MenuItem::Action { label, enabled, .. }) => {
                assert_eq!(label, "打开主界面");
                assert!(*enabled);
            }
            other => panic!("应找到 open 动作项，实得 {other:?}"),
        }
    }

    #[test]
    fn tray_menu_toggle_and_set_enabled() {
        let mut menu = TrayMenu::new()
            .checkbox("autostart", "开机自启", false)
            .action("open", "打开");
        // 切换勾选项，返回新值。
        assert_eq!(menu.toggle("autostart"), Some(true));
        assert_eq!(menu.toggle("autostart"), Some(false));
        // 对非勾选项 / 不存在项返回 None。
        assert_eq!(menu.toggle("open"), None);
        assert_eq!(menu.toggle("ghost"), None);
        // 置灰动作项。
        assert!(menu.set_enabled("open", false));
        assert!(!menu.set_enabled("ghost", false));
        assert!(matches!(
            menu.find("open"),
            Some(MenuItem::Action { enabled: false, .. })
        ));
    }

    #[test]
    fn menu_renders_to_html() {
        let menu = TrayMenu::new()
            .action("open", "打开")
            .checkbox("autostart", "自启", true)
            .separator();
        let html = menu.to_html();
        assert!(html.contains("data-id=\"open\""));
        assert!(html.contains("[x] 自启")); // checked 勾选项
        assert!(html.contains("<hr>")); // 分隔线
    }

    #[test]
    fn config_validation_and_autostart_metadata() {
        let mut cfg = AppConfig::new("LenovoAgent");
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.autostart_value_name(), "LenovoAgent");
        assert_eq!(
            AUTOSTART_REG_PATH,
            r"Software\Microsoft\Windows\CurrentVersion\Run"
        );

        // 空名 / 非法字符应被拒绝。
        cfg.app_name = "   ".into();
        assert_eq!(cfg.validate(), Err(ConfigError::EmptyName));
        cfg.app_name = r"bad\name".into();
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidNameChar('\\')));
    }
}
