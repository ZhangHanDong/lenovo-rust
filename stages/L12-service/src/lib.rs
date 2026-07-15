//! L12 作业：WinMon 服务的 typestate 状态机
//!
//! 服务有三个状态：Stopped / Running / Paused。合法转移只有几条：
//!   Stopped --start--> Running --pause--> Paused --resume--> Running
//!   Running/Paused --stop--> Stopped
//!
//! **非法转移（如在 Stopped 上 resume）应该编译不过**——用类型状态（typestate）
//! 把状态编码进类型参数，非法转移的方法根本不存在。这是纯类型层面的设计，
//! 平台无关、完全可测。真实的 `sc create` / toast 见 SOLUTION.md（Windows 上跑）。

use std::marker::PhantomData;

pub struct Stopped;
pub struct Running;
pub struct Paused;

/// 服务句柄，状态编码在类型参数 `S` 里。
///
/// 非法转移编译不过——typestate 的核心保证。`Service<Stopped>` 上没有 `resume`
/// 方法（它只在 `Service<Paused>` 上），也没有 `pause`（只在 `Running` 上）：
///
/// ```compile_fail
/// use winmon_l12_service::{Service, Stopped};
/// let s = Service::<Stopped>::new("WinMon");
/// let _ = s.resume();   // ✗ Service<Stopped> 没有 resume 方法
/// ```
///
/// ```compile_fail
/// use winmon_l12_service::{Service, Stopped};
/// let s = Service::<Stopped>::new("WinMon");
/// let _ = s.pause();    // ✗ 只有 Running 能 pause
/// ```
pub struct Service<S> {
    name: String,
    _state: PhantomData<S>,
}

impl<S> Service<S> {
    pub fn name(&self) -> &str {
        &self.name
    }
    fn transition<T>(self) -> Service<T> {
        Service {
            name: self.name,
            _state: PhantomData,
        }
    }
}

impl Service<Stopped> {
    pub fn new(name: impl Into<String>) -> Self {
        Service {
            name: name.into(),
            _state: PhantomData,
        }
    }

    /// 启动：Stopped → Running。（真实实现里这里调 `sc start` / 服务主循环。）
    pub fn start(self) -> Service<Running> {
        todo!("L12：Stopped → Running 的类型状态转移")
    }
}

impl Service<Running> {
    pub fn pause(self) -> Service<Paused> {
        todo!("L12：Running → Paused")
    }
    pub fn stop(self) -> Service<Stopped> {
        self.transition()
    }
}

impl Service<Paused> {
    pub fn resume(self) -> Service<Running> {
        todo!("L12：Paused → Running")
    }
    pub fn stop(self) -> Service<Stopped> {
        self.transition()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合法转移链：Stopped → Running → Paused → Running → Stopped。
    #[test]
    fn legal_transitions_compile_and_run() {
        let s = Service::<Stopped>::new("WinMon");
        assert_eq!(s.name(), "WinMon");
        let s = s.start(); // Running
        let s = s.pause(); // Paused
        let s = s.resume(); // Running
        let _s = s.stop(); // Stopped
    }
}
