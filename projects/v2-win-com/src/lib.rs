//! 第 10 课配套工程：COM / WinRT 深入。
//!
//! 本 crate 沿用第 13 课的跨平台纪律，切成两层：
//!
//! - **平台无关核心**（本文件顶层）：把「一次 COM 调用的结果」抽象成
//!   [`Hresult`]（HRESULT newtype，可分解 severity/facility/code、可分类）与
//!   [`CallRecord`]（接口名 + 方法 + HRESULT），并提供纯函数
//!   [`format_call_report`]（把一串 COM 调用结果排版成报告）。另外用
//!   [`RefCountLedger`] 在纯逻辑层模拟 COM 的 AddRef/Release 配平——这正是
//!   C++ 里你要手工盯防、而 Rust 用 `Clone=AddRef` / `Drop=Release` 自动维护的纪律。
//!   这些逻辑不碰任何系统 API，**在 macOS / Linux / Windows 上行为一致、可单测**。
//! - **Windows COM 适配层**（[`mod@com`]，`#[cfg(windows)]`）：用 `windows` crate
//!   `CoInitializeEx` 进入 STA 公寓、`CoCreateInstance` 消费 Shell 的 `IShellLinkW`、
//!   `cast`（QueryInterface）转到 `IPersistFile`、用 `#[implement]` 把一个 Rust 类型
//!   导出为最小 COM 对象（实现 `IPersist`）并回调它，最后演示 WinRT `HSTRING`。
//!   每一步都把结果收敛成核心层的 [`CallRecord`]。
//! - **非 Windows 桩**（[`sample_call_log`]）：返回一份样例调用记录，使
//!   [`com_support_summary`] 在 macOS / Linux 桌面也能跑通核心排版逻辑。
//!
//! 关键工程取舍：`windows` 依赖写在本 crate 自己 `Cargo.toml` 的
//! `[target.'cfg(windows)'.dependencies]` 下，**交付机（macOS）根本不会编译 windows crate**。
//! COM / WinRT 实际代码全部隔离在 `#[cfg(windows)] mod com` 之后，不拖累其它平台构建。
//!
//! 赏析锚点：`windows-core` 如何把 `IUnknown` / 引用计数 / `Interface` 表达进类型系统，
//! `#[implement]` 如何为 Rust 类型生成 vtable —— 见 <https://github.com/microsoft/windows-rs>。

use std::fmt;

/// 一个 HRESULT 码（COM 调用的统一返回类型）。
///
/// HRESULT 是 32 位有符号整数，**最高位即 severity**：`< 0` 表示失败（`FAILED`）、
/// `>= 0` 表示成功（`SUCCEEDED`）。这正是 `windows::core::HRESULT` 内部 `.0` 的语义，
/// 我们在核心层用一个零成本 newtype 复刻它，便于在桌面单测分解/分类逻辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hresult(pub i32);

impl Hresult {
    /// `S_OK`：调用成功。
    pub const S_OK: Hresult = Hresult(0);
    /// `S_FALSE`：成功，但语义上「没有发生操作 / 条件为假」。
    pub const S_FALSE: Hresult = Hresult(1);
    /// `E_NOINTERFACE`：对象不支持所请求的接口（`QueryInterface` 最常见的失败）。
    pub const E_NOINTERFACE: Hresult = Hresult(0x8000_4002_u32 as i32);
    /// `E_POINTER`：传入了非法（通常是空）指针。
    pub const E_POINTER: Hresult = Hresult(0x8000_4003_u32 as i32);
    /// `E_FAIL`：未指明的失败。
    pub const E_FAIL: Hresult = Hresult(0x8000_4005_u32 as i32);
    /// `E_ACCESSDENIED`：拒绝访问（FACILITY_WIN32，code = 5）。
    pub const E_ACCESSDENIED: Hresult = Hresult(0x8007_0005_u32 as i32);
    /// `RPC_E_CHANGED_MODE`：本线程已用不同的公寓模型初始化过 COM。
    pub const RPC_E_CHANGED_MODE: Hresult = Hresult(0x8001_0106_u32 as i32);

    /// 是否成功（`SUCCEEDED(hr)`，即 `hr >= 0`）。
    #[must_use]
    pub const fn is_ok(self) -> bool {
        self.0 >= 0
    }

    /// 是否失败（`FAILED(hr)`，即 `hr < 0`）。
    #[must_use]
    pub const fn is_err(self) -> bool {
        self.0 < 0
    }

    /// severity 位（bit 31）：失败为 1、成功为 0。
    #[must_use]
    pub const fn severity(self) -> u8 {
        ((self.0 as u32 >> 31) & 0x1) as u8
    }

    /// facility 段（bit 16..=26，共 11 位）：错误来源子系统，如 `FACILITY_WIN32 = 7`。
    #[must_use]
    pub const fn facility(self) -> u16 {
        ((self.0 as u32 >> 16) & 0x7FF) as u16
    }

    /// code 段（低 16 位）：具体错误码。
    #[must_use]
    pub const fn code(self) -> u16 {
        (self.0 as u32 & 0xFFFF) as u16
    }

    /// 把已知 HRESULT 映射成可读说明；未具名时按成功/失败给出兜底文案。
    #[must_use]
    pub fn classify(self) -> &'static str {
        if self == Self::S_OK {
            "S_OK 调用成功"
        } else if self == Self::S_FALSE {
            "S_FALSE 成功但未发生操作"
        } else if self == Self::E_NOINTERFACE {
            "E_NOINTERFACE 不支持该接口（QueryInterface 失败）"
        } else if self == Self::E_POINTER {
            "E_POINTER 非法指针"
        } else if self == Self::E_FAIL {
            "E_FAIL 未指明的失败"
        } else if self == Self::E_ACCESSDENIED {
            "E_ACCESSDENIED 拒绝访问"
        } else if self == Self::RPC_E_CHANGED_MODE {
            "RPC_E_CHANGED_MODE 公寓模型冲突"
        } else if self.is_ok() {
            "成功（未具名 HRESULT）"
        } else {
            "失败（未具名 HRESULT）"
        }
    }
}

impl fmt::Display for Hresult {
    /// 按 COM 习惯打印成 8 位十六进制，如 `0x80070005`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08X}", self.0 as u32)
    }
}

/// 一条 COM 调用记录：调用了哪个接口的哪个方法、返回的 HRESULT。
///
/// 这是 Windows 适配层与桩共同收敛的领域类型——核心排版逻辑只认它，
/// 不认任何 `windows` crate 的具体接口，从而能在桌面独立测试。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    /// 接口名（如 `IShellLinkW`）。
    pub interface: String,
    /// 方法名（如 `CoCreateInstance`）。
    pub method: String,
    /// 该调用返回的 HRESULT。
    pub hr: Hresult,
}

impl CallRecord {
    /// 通用构造。
    #[must_use]
    pub fn new(interface: impl Into<String>, method: impl Into<String>, hr: Hresult) -> Self {
        Self {
            interface: interface.into(),
            method: method.into(),
            hr,
        }
    }

    /// 记一条成功调用（`S_OK`）。
    #[must_use]
    pub fn ok(interface: impl Into<String>, method: impl Into<String>) -> Self {
        Self::new(interface, method, Hresult::S_OK)
    }

    /// 记一条失败调用。
    #[must_use]
    pub fn failed(interface: impl Into<String>, method: impl Into<String>, hr: Hresult) -> Self {
        Self::new(interface, method, hr)
    }

    /// 该调用是否成功。
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.hr.is_ok()
    }
}

/// 把一串 COM 调用记录排版成对齐的文本报告。纯函数，便于快照式断言。
#[must_use]
pub fn format_call_report(records: &[CallRecord]) -> String {
    let mut out = String::from("INTERFACE        METHOD               HRESULT      STATUS\n");
    for r in records {
        let hr = r.hr.to_string();
        let status = if r.succeeded() { "OK" } else { "ERR" };
        out.push_str(&format!(
            "{:<16} {:<20} {hr:<12} {status}\n",
            r.interface, r.method
        ));
    }
    out
}

/// 在纯逻辑层模拟 COM 对象的引用计数（AddRef / Release）。
///
/// COM 的生命周期规则：创建时引用计数为 1，每次 `AddRef` +1、每次 `Release` -1，
/// 减到 0 时对象自毁。C++ 里你必须**手工配平**——漏一次 `Release` 就泄漏、
/// 多一次就提前析构（UAF）。Rust 用 `Clone=AddRef` / `Drop=Release` 把这条纪律
/// 变成所有权自动维护；本类型只是把「手工记账」这件事显式建模出来，用于教学与测试。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefCountLedger {
    count: i64,
}

impl RefCountLedger {
    /// 新建对象：引用计数初始化为 1（对照 COM 对象「创建即持有一个引用」）。
    #[must_use]
    pub fn new() -> Self {
        Self { count: 1 }
    }

    /// `AddRef`：引用计数 +1，返回新值（COM 的 `AddRef` 返回 `ULONG`）。
    pub fn add_ref(&mut self) -> i64 {
        self.count += 1;
        self.count
    }

    /// `Release`：引用计数 -1，返回新值。
    pub fn release(&mut self) -> i64 {
        self.count -= 1;
        self.count
    }

    /// 当前引用计数。
    #[must_use]
    pub fn count(&self) -> i64 {
        self.count
    }

    /// 是否配平（计数恰好归零，对象已正确释放）。
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.count == 0
    }

    /// 是否泄漏（计数仍 > 0：有 `AddRef` 没配对 `Release`）。
    #[must_use]
    pub fn leaked(&self) -> bool {
        self.count > 0
    }

    /// 是否过度释放（计数 < 0：`Release` 次数多于 `AddRef`，对应 C++ 的 UAF 风险）。
    #[must_use]
    pub fn over_released(&self) -> bool {
        self.count < 0
    }
}

impl Default for RefCountLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// 一份样例 COM 调用记录：用于非 Windows 桩与桌面测试，
/// 让「平台无关核心」无需真实 COM 也能被验证。
#[must_use]
pub fn sample_call_log() -> Vec<CallRecord> {
    vec![
        CallRecord::ok("IShellLinkW", "CoCreateInstance"),
        CallRecord::ok("IShellLinkW", "SetPath"),
        CallRecord::ok("IPersistFile", "QueryInterface"),
        CallRecord::ok("IPersist", "GetClassID(impl)"),
        CallRecord::failed("IFoo", "QueryInterface", Hresult::E_NOINTERFACE),
    ]
}

/// 核心与平台层的唯一汇合点：返回一段「COM 支持情况」报告。
///
/// - 在 Windows 上：转发到 [`mod@com`]，真正初始化 COM、消费 Shell 接口、回调自实现的
///   COM 对象，并把每步结果排版成报告。
/// - 在其它平台上：用 [`sample_call_log`] 的样例数据走一遍核心排版逻辑，证明核心可独立验证。
#[must_use]
pub fn com_support_summary() -> String {
    #[cfg(windows)]
    {
        match com::probe() {
            Ok(log) => format_call_report(&log),
            Err(e) => format!("COM 探测失败：{e}"),
        }
    }
    #[cfg(not(windows))]
    {
        let sample = sample_call_log();
        format!(
            "COM / WinRT 仅在 Windows 可用；以下为样例报告（桩）：\n{}",
            format_call_report(&sample)
        )
    }
}

/// Windows COM / WinRT 适配层：用 `windows` crate 消费并实现 COM 接口。
///
/// 整个模块在 `#[cfg(windows)]` 之后，macOS / Linux 构建时**不参与编译**，
/// 对 `windows` crate 的依赖也只在 Windows 目标被拉取。
#[cfg(windows)]
mod com {
    use super::{CallRecord, Hresult};
    use windows::core::{implement, w, Interface, Result, GUID, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersist, IPersistFile, IPersist_Impl,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    /// 我们自定义 COM 对象的 CLSID（教学用，随便造一个稳定的 GUID）。
    const GREETER_CLSID: GUID = GUID::from_u128(0x00112233_4455_6677_8899_aabbccddeeff);

    /// 用 `#[implement]` 把一个 Rust 类型导出为 COM 对象。
    ///
    /// `#[implement(IPersist)]` 让宏为 `Greeter` 生成 `IPersist`（其基类 `IUnknown`）的
    /// **vtable**，并自动实现 `AddRef`/`Release`/`QueryInterface`——这正是 C++ 里你要靠
    /// ATL/WRL 模板或手写一大坨样板才能得到的东西。我们只需实现接口的业务方法。
    #[implement(IPersist)]
    struct Greeter;

    // 实现 `IPersist` 的业务方法。`#[implement]` 生成的 `IPersist_Impl` trait 把
    // 「该接口需要我们填的方法」表达成 Rust trait；这里只有一个 `GetClassID`。
    impl IPersist_Impl for Greeter_Impl {
        fn GetClassID(&self) -> Result<GUID> {
            Ok(GREETER_CLSID)
        }
    }

    /// COM 公寓 RAII 守卫：作用域结束自动 `CoUninitialize`，与 `CoInitializeEx` 配平。
    /// 对照 C++ 里手写「函数出口处别忘了 CoUninitialize」的纪律——任何 `?` 早退都安全。
    struct ComApartment;

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: 仅在 CoInitializeEx 成功后构造本守卫，故此处与之严格配平、只调用一次。
            unsafe { CoUninitialize() };
        }
    }

    /// 真正跑一遍「消费 COM + 实现 COM + WinRT」全链路，把每步结果收敛成核心层的 `CallRecord`。
    pub(super) fn probe() -> Result<Vec<CallRecord>> {
        let mut log = Vec::new();

        // 1) 进入 STA 公寓。CoInitializeEx 返回 HRESULT，用 `.ok()?` 转成 Result 并用 `?` 传播。
        // SAFETY: 首次在本线程初始化 COM；返回值经 ok()? 校验，随即用 RAII 守卫保证配平。
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        let _apartment = ComApartment; // ← 之后任何 ? 早退都会 CoUninitialize
        log.push(CallRecord::ok("CoInitializeEx", "STA"));

        // 2) 消费 COM：CoCreateInstance 创建 Shell 的 IShellLinkW。
        // SAFETY: ShellLink 为进程内服务，T=IShellLinkW 与之匹配；失败时 ? 直接返回（守卫仍会清理）。
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };
        log.push(CallRecord::ok("IShellLinkW", "CoCreateInstance"));

        // 3) 调接口方法：设置快捷方式目标路径。COM 方法是 unsafe 的。
        // SAFETY: link 为有效接口指针；w! 产出以 NUL 结尾的 UTF-16 字面量，生命周期覆盖调用。
        unsafe { link.SetPath(w!("C:\\Windows\\explorer.exe"))? };
        log.push(CallRecord::ok("IShellLinkW", "SetPath"));

        // 4) QueryInterface：`cast` 是 windows-rs 对 QueryInterface 的安全封装，
        //    成功返回新接口（已 AddRef，Drop 时自动 Release），失败返回 Err(E_NOINTERFACE)。
        let _persist_file: IPersistFile = link.cast()?;
        log.push(CallRecord::ok("IPersistFile", "QueryInterface(cast)"));

        // 5) 实现 COM：把自定义 Greeter 实例化为 IPersist 接口并回调它。
        //    `.into()` 由 #[implement] 生成：构造 COM 对象、返回首个接口指针（引用计数=1）。
        let greeter: IPersist = Greeter.into();
        // SAFETY: greeter 为我们刚构造的有效 COM 对象，GetClassID 不接触外部资源。
        let clsid: GUID = unsafe { greeter.GetClassID()? };
        debug_assert_eq!(clsid, GREETER_CLSID);
        log.push(CallRecord::ok("IPersist", "GetClassID(impl)"));

        // 6) WinRT：HSTRING 是引用计数的宽字符串，演示与 Rust String 的互转。
        let hs = HSTRING::from("Rust ❤ WinRT");
        let round_trip = hs.to_string();
        debug_assert_eq!(round_trip, "Rust ❤ WinRT");
        log.push(CallRecord::ok("HSTRING", "from/to_string"));

        Ok(log)
        // 此处 link / greeter / _persist_file 依次 Drop → 各自 Release；
        // _apartment Drop → CoUninitialize。全部由所有权自动配平，无需手写。
    }

    /// 把 `windows::core::HRESULT` 转成核心层的 [`Hresult`]（两者内部都是 i32，零成本）。
    /// 留作适配层把 windows 错误码翻译给核心层分类/排版时使用。
    #[allow(dead_code)]
    pub(super) fn to_core_hresult(hr: windows::core::HRESULT) -> Hresult {
        Hresult(hr.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_classifies_success_and_failure() {
        assert!(Hresult::S_OK.is_ok());
        assert!(Hresult::S_FALSE.is_ok()); // 1 >= 0，成功
        assert!(Hresult::E_NOINTERFACE.is_err());
        assert!(Hresult::E_ACCESSDENIED.is_err());
        assert!(Hresult::E_NOINTERFACE.classify().contains("E_NOINTERFACE"));
        assert!(Hresult::S_OK.classify().contains("S_OK"));
        // 未具名失败码也能兜底分类
        assert!(Hresult(0x8000_FFFF_u32 as i32).classify().contains("失败"));
    }

    #[test]
    fn hresult_decomposes_into_severity_facility_code() {
        let hr = Hresult::E_ACCESSDENIED; // 0x80070005
        assert_eq!(hr.severity(), 1);
        assert_eq!(hr.facility(), 7); // FACILITY_WIN32
        assert_eq!(hr.code(), 5); // ERROR_ACCESS_DENIED
        assert_eq!(hr.to_string(), "0x80070005");
        assert_eq!(Hresult::S_OK.severity(), 0);
    }

    #[test]
    fn refcount_ledger_tracks_balance_leak_and_over_release() {
        // 配平：创建(1) + AddRef(2) - Release - Release = 0
        let mut l = RefCountLedger::new();
        assert_eq!(l.count(), 1);
        assert_eq!(l.add_ref(), 2);
        l.release();
        l.release();
        assert!(l.is_balanced());
        assert!(!l.leaked());

        // 泄漏：AddRef 没配对 Release
        let mut leaky = RefCountLedger::new();
        leaky.add_ref();
        leaky.release();
        assert!(leaky.leaked());
        assert_eq!(leaky.count(), 1);

        // 过度释放：Release 多于 AddRef（C++ 的 UAF 温床）
        let mut over = RefCountLedger::new();
        over.release();
        over.release();
        assert!(over.over_released());
    }

    #[test]
    fn format_call_report_has_header_and_one_row_per_call() {
        let log = sample_call_log();
        let table = format_call_report(&log);
        assert!(table.starts_with("INTERFACE"));
        // 1 行表头 + N 行数据
        assert_eq!(table.lines().count(), 1 + log.len());
        assert!(table.contains("IShellLinkW"));
        assert!(table.contains("OK"));
        assert!(table.contains("ERR")); // 样例里最后一条是 E_NOINTERFACE
    }

    #[test]
    fn com_support_summary_is_callable_on_every_platform() {
        // macOS 走桩、Windows 走真实 COM 链路；两者都应返回一段非空报告。
        let summary = com_support_summary();
        assert!(!summary.is_empty());
    }
}
