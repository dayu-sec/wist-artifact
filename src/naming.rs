//! 制品命名与寻址：内容寻址 id、落盘/下发文件名、路径段安全。

/// 内容寻址 id：`<prefix>-<sha256 前 16 位>`（裸 hex）。
///
/// 取前 16 位（64 bit）足够区分一台机器里录入过的制品，同时保持文件名短、可读。
/// 前缀由调用方给（安装包 `pkg`、知识库包 `kbp`）—— 同一套取 id 的口径，不各写一遍。
pub fn content_id(prefix: &str, sha256_hex: &str) -> String {
    let digest_prefix: String = sha256_hex.chars().take(16).collect();
    format!("{prefix}-{digest_prefix}")
}

/// 安装包的内容寻址 id：`pkg-<sha256 前 16 位>`（裸 hex）。= `content_id("pkg", …)`。
pub fn package_id_for_sha256(sha256_hex: &str) -> String {
    content_id("pkg", sha256_hex)
}

/// 一个**路径段**是否安全：非空、不是 `.` / `..`、不含路径分隔符与控制字符。
///
/// 用在「组件名 / 版本号 / 制品文件名」这类要拼进目录或 URL 的字段上 —— 它们必须真的只有一段，
/// 否则 `Path::join`（以及对象存储的 key）会顺着 `..` 或绝对路径逃出目标目录。
/// 这是**校验**侧入口（调用方拿它直接 400/404）；[`artifact_filename`] 的回落名另有兜底。
pub fn is_safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
}

/// 把任意输入压成一个**安全的路径段**：分隔符 / 控制字符换成 `-`，空 / `.` / `..` 换成 `_`。
///
/// 给**不能失败**的地方兜底（[`artifact_filename`] 的回落名），保证本模块永远不会产出一个能逃出
/// 目录的段。要「拒绝非法输入」请用 [`is_safe_path_segment`]。
fn sanitize_segment(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch == '/' || ch == '\\' || ch.is_control() {
                '-'
            } else {
                ch
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return "_".to_string();
    }
    cleaned
}

/// 制品文件名：取来源（路径 / URL）的**末段原名**（去掉查询串 / fragment）。
///
/// 用**原文件名**而非内容寻址 id：下发 URL 的末段就是原名，人看着清楚、下载即得可用文件。
/// 末段不安全（空、`.`、`..`、含分隔符或控制字符）就回落 `{component}-{version}.bin`。
///
/// 两种情况下返回值**都**保证是一个安全路径段：正常分支靠 [`is_safe_path_segment`] 把关，
/// 回落分支把 `component` / `version` 压干净（见 `sanitize_segment`）—— 也就是说即便调用方
/// 忘了校验入参，本函数也不会吐出一个能逃出目录的名字。
pub fn artifact_filename(url: &str, component: &str, version: &str) -> String {
    let leading = url.split(['?', '#']).next().unwrap_or(url);
    let basename = leading.rsplit('/').next().unwrap_or("");
    if is_safe_path_segment(basename) {
        return basename.to_string();
    }
    format!(
        "{}-{}.bin",
        sanitize_segment(component),
        sanitize_segment(version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_id_is_the_prefix_plus_sixteen_hex_chars() {
        let sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(content_id("kbp", sha), "kbp-0123456789abcdef");
        assert_eq!(content_id("pkg", sha), package_id_for_sha256(sha));
        // 前缀原样用（不归一大小写、不额外加分隔符）；短摘要不补齐。
        assert_eq!(content_id("X", sha), "X-0123456789abcdef");
        assert_eq!(content_id("", sha), "-0123456789abcdef");
        assert_eq!(content_id("kbp", "abc"), "kbp-abc");
    }

    #[test]
    fn package_id_is_stable_prefixed_and_digest_sized() {
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let id = package_id_for_sha256(digest);
        assert_eq!(id, "pkg-0123456789abcdef");
        assert_eq!(id.len(), "pkg-".len() + 16);
        // 同一 sha256 两次得到同一 id（幂等，内容寻址的前提）。
        assert_eq!(package_id_for_sha256(digest), id);
        assert_ne!(
            package_id_for_sha256(
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
            ),
            id
        );
    }

    #[test]
    fn package_id_takes_sixteen_chars_and_does_not_normalize_case() {
        // 短输入不补齐；大小写不归一（上游给的就是裸 hex 小写，本函数不替它收拾）。
        assert_eq!(package_id_for_sha256("abc"), "pkg-abc");
        assert_eq!(package_id_for_sha256(""), "pkg-");
        assert_eq!(
            package_id_for_sha256("AB0123456789ABCDEF99"),
            "pkg-AB0123456789ABCD"
        );
        assert_ne!(
            package_id_for_sha256("AB0123456789ABCDEF99"),
            package_id_for_sha256("ab0123456789abcdef99")
        );
    }

    #[test]
    fn is_safe_path_segment_accepts_plain_names_and_rejects_escaping_ones() {
        for safe in [
            "pkg",
            "wist-gateway-stack",
            "0.1.0-rc.1",
            ".hidden",
            "a b",
            "….gz",
        ] {
            assert!(is_safe_path_segment(safe), "{safe:?} 应算安全");
        }
        for unsafe_value in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "../x",
            "./x",
            "/etc/passwd",
            "a\nb",
            "a\0b",
            "\u{7f}",
        ] {
            assert!(
                !is_safe_path_segment(unsafe_value),
                "{unsafe_value:?} 应算不安全"
            );
        }
    }

    #[test]
    fn artifact_filename_uses_the_source_basename_and_guards_paths() {
        assert_eq!(
            artifact_filename(
                "https://github.com/galaxio-labs/galaxy-flow/releases/download/v0.16.1-alpha/galaxy-flow-v0.16.1-alpha-x86_64-unknown-linux-musl.tar.gz",
                "galaxy-flow",
                "v0.16.1-alpha",
            ),
            "galaxy-flow-v0.16.1-alpha-x86_64-unknown-linux-musl.tar.gz"
        );
        assert_eq!(
            artifact_filename("https://x/pkg.tar.gz?sig=1#frag", "c", "1.0"),
            "pkg.tar.gz"
        );
        assert_eq!(
            artifact_filename("https://x/dir/", "galaxy-ops", "1.2.3"),
            "galaxy-ops-1.2.3.bin"
        );
        assert_eq!(
            artifact_filename("/opt/pkgs/..", "galaxy-ops", "1.2.3"),
            "galaxy-ops-1.2.3.bin"
        );
    }

    #[test]
    fn artifact_filename_never_returns_an_escaping_segment() {
        // 来源末段不安全 → 回落；回落名也**保证**是单段（分隔符 / `..` 被压掉）。
        assert_eq!(
            artifact_filename("https://x/dir/", "../etc", "../passwd"),
            "..-etc-..-passwd.bin"
        );
        // 反斜杠 / 控制字符也算危险末段 → 回落。
        assert_eq!(
            artifact_filename("https://x/..\\evil", "galaxy-ops", "1.2.3"),
            "galaxy-ops-1.2.3.bin"
        );
        assert_eq!(
            artifact_filename("https://x/bad\nname", "galaxy-ops", "1.2.3"),
            "galaxy-ops-1.2.3.bin"
        );
        // 回落名的两段都不可用时用 `_` 占位，仍是单段。
        assert_eq!(artifact_filename("https://x/dir/", "..", "."), "_-_.bin");
        // 正常末段照旧原样用。
        assert_eq!(
            artifact_filename("https://x/dir/pkg.tar.gz", "c", "1.0"),
            "pkg.tar.gz"
        );
    }
}
