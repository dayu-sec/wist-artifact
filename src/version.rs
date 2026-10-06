//! 版本号口径：归一、拆段、比新旧。
//!
//! 「该不该升 / 是不是降级」在**被管端**（agentd 决定升不升）与**控制面**（挑可升级版本）都要判，
//! 判据只能有一份 —— 判错方向就是换了个更旧的版本上去。

/// 版本比对用归一：忽略首尾空白与可选的 `v` / `V` 前缀（包内自报常按 git tag 带 `v`）。
///
/// 只剥**一个**前缀：`v`/`V` 是 git tag 惯例，`vv1.2` 不是合法版本写法；大小写都认 ——
/// 否则包内自报的 `V1.2` 与运维手输的 `v1.2` 归一后不等，会被判成版本不符。
pub fn normalize_version(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix(['v', 'V'])
        .unwrap_or(trimmed)
        .to_string()
}

/// 版本号拆成可比的分段（`0.1.4` → `[0,1,4]`）。带 `-pre` / `+meta` 后缀时只看前面的数字段。
///
/// 认不出来就返回 `None` —— 比较方向**宁可拒绝，也不要拿字符串去猜大小**。
/// 注意这里**不剥 `v` 前缀**：`v0.1.4` 属于「认不出来的版本」（要不要当合法写法由调用方决定，
/// 例如先用 [`normalize_version`] 归一）。
pub fn parse_version(value: &str) -> Option<Vec<u64>> {
    let core = value.trim().split(['-', '+']).next()?;
    if core.is_empty() {
        return None;
    }
    core.split('.')
        .map(|segment| segment.parse::<u64>().ok())
        .collect()
}

/// 目标版本是否比当前版本**新**（按数字段逐段比，`0.10 > 0.9`、`0.1.0 > 0.1`）。
///
/// 两边任一认不出来就 `None`（见 [`parse_version`]）—— 升级方向不可判时，调用方应当**拒绝**
/// 而不是猜。
pub fn version_is_newer(target: &str, current: &str) -> Option<bool> {
    Some(parse_version(target)? > parse_version(current)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_version_strips_one_v_or_capital_v_prefix() {
        assert_eq!(normalize_version("v0.1.32"), "0.1.32");
        assert_eq!(normalize_version("  V0.1.32  "), "0.1.32");
        assert_eq!(normalize_version("0.1.32"), "0.1.32");
        // 只剥一个前缀：`vv1.2` 不是合法版本写法，不该被当成 `1.2`。
        assert_eq!(normalize_version("vv1.2"), "v1.2");
        // 包内自报带 `v`、运维手输不带（或大小写不同）时，归一后必须相等。
        assert_eq!(
            normalize_version("v0.16.1-alpha"),
            normalize_version("0.16.1-alpha")
        );
        assert_eq!(normalize_version("V0.16.1"), normalize_version("v0.16.1"));
    }

    #[test]
    fn normalize_version_ignores_whitespace_and_leading_v() {
        assert_eq!(normalize_version(" v0.1.32 "), normalize_version("0.1.32"));
        assert_ne!(normalize_version("0.1.32"), normalize_version("0.1.33"));
    }

    #[test]
    fn version_is_newer_follows_the_documented_rules() {
        // 数值比较（不是字典序）：0.1.10 比 0.1.9 新。
        assert_eq!(version_is_newer("0.1.10", "0.1.9"), Some(true));
        assert_eq!(version_is_newer("0.1.9", "0.1.10"), Some(false));
        // 相等 → Some(false)：同版本重装不算「更新」。
        assert_eq!(version_is_newer("0.1.9", "0.1.9"), Some(false));
        // 预发布 / 构建后缀只看前面的数字段。
        assert_eq!(version_is_newer("0.2.0-beta.1", "0.1.9"), Some(true));
        assert_eq!(version_is_newer("0.1.4+meta", "0.1.3"), Some(true));
        // 段逐段比、不补齐：四段比三段新，`0.1.0` 比 `0.1` 新。
        assert_eq!(version_is_newer("0.1.9.1", "0.1.9"), Some(true));
        assert_eq!(version_is_newer("0.1.0", "0.1"), Some(true));
        // 认不出来就 None（`v` 前缀不算数字段）。
        for (target, current) in [
            ("dev", "0.1.9"),
            ("0.1.9", "dev"),
            ("v0.1.9", "0.1.9"),
            ("abc", "0.1.9"),
            ("0.1.9", ""),
            ("", "0.1.9"),
            ("0.1.9", "0.1.x"),
        ] {
            assert_eq!(
                version_is_newer(target, current),
                None,
                "{target:?} vs {current:?}"
            );
        }
        // 要认 `v` 前缀就由调用方先归一。
        let target = normalize_version("v0.1.9");
        let current = normalize_version("0.1.8");
        assert_eq!(version_is_newer(&target, &current), Some(true));
    }
}
