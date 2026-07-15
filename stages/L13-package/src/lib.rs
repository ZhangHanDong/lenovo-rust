//! L13 作业：让 CI 全绿 + MSI 可安装
//!
//! 这课的主体是 CI（三 job：gate / cross-build / windows-package）和 WiX 配置——
//! 它们是**交付物文件**（参考见 SOLUTION.md），不参与 Rust 编译。
//!
//! 这里放**可测的那部分**：版本号解析 + MSI 产物命名。CI 用它给产物打上一致的名字。

/// 语义版本 `major.minor.patch`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// 解析 `"1.0.3"`。格式不对返回 `None`（不 panic）。
    pub fn parse(s: &str) -> Option<Version> {
        todo!("L13 解析三段式版本号，任一段非法返回 None（不 panic）")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// CI 产物命名：`winmon-1.0.0-x64.msi`——版本 + 架构一致，方便 Release 归档。
pub fn msi_filename(version: Version, arch: &str) -> String {
    todo!("L13 拼出 winmon 版本 架构 组成的 .msi 文件名")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_version() {
        assert_eq!(
            Version::parse("1.0.3"),
            Some(Version {
                major: 1,
                minor: 0,
                patch: 3
            })
        );
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(Version::parse("1.0"), None); // 缺 patch
        assert_eq!(Version::parse("1.0.0.0"), None); // 多一段
        assert_eq!(Version::parse("1.x.0"), None); // 非数字
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn msi_name_is_consistent() {
        let v = Version::parse("1.0.0").unwrap();
        assert_eq!(msi_filename(v, "x64"), "winmon-1.0.0-x64.msi");
    }
}
