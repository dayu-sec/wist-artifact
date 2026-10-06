//! 摘要口径：`sha256` 的裸 hex 与「期望摘要」字符串的解析。
//!
//! 用 `sha2`（纯 Rust）而不是 `ring`：不把 C/汇编依赖带给每一个消费方 —— 与
//! `wist-contracts::fact_summary` 选 `sha2` 的理由一致。sha256 的输出与实现无关，
//! 两侧换来换去不会让摘要对不上。

/// 字节 sha256（**裸 hex、小写**，不带 `sha256:` 前缀）。
pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex_lower(&Sha256::digest(bytes))
}

/// 期望摘要的**规范形态**：可选 `sha256:` 前缀（大小写不敏感、容忍两侧空格）、裸 hex 大小写不敏感；
/// 但必须是 **64 位 hex**。返回小写裸 hex。
///
/// 长度也校验，是因为「填错一个摘要」几乎总是笔误：早一步说「这不是摘要」，比让它跑完全程
/// 再报一句 mismatch 可操作得多。
pub fn parse_digest(value: &str) -> Result<String, String> {
    let stripped = strip_digest_prefix(value);
    let looks_like_hex = stripped.len() == 64 && stripped.chars().all(|ch| ch.is_ascii_hexdigit());
    if !looks_like_hex {
        return Err(format!(
            "digest must be 64 hex chars (an optional `sha256:` prefix is allowed), got {value:?}"
        ));
    }
    Ok(stripped.to_ascii_lowercase())
}

/// 去掉可选的 `sha256:` 前缀（大小写不敏感，两侧空格容忍）；只认 `sha256` 这一种 scheme，
/// 别的（或没有冒号的裸 hex）原样返回。
fn strip_digest_prefix(value: &str) -> &str {
    let trimmed = value.trim();
    match trimmed.split_once(':') {
        Some((scheme, rest)) if scheme.trim().eq_ignore_ascii_case("sha256") => rest.trim(),
        _ => trimmed,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_bytes_matches_known_vectors() {
        assert_eq!(
            sha256_hex_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_bytes_is_lowercase_hex_of_the_expected_length() {
        let digest = sha256_hex_bytes(b"");
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(!digest.chars().any(|ch| ch.is_ascii_uppercase()));
    }

    #[test]
    fn parse_digest_normalizes_the_documented_forms_and_rejects_the_rest() {
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let upper = digest.to_ascii_uppercase();
        for value in [
            digest.to_string(),
            upper.clone(),
            format!("sha256:{digest}"),
            format!("SHA256:{upper}"),
            format!(" sha256: {upper} "),
        ] {
            assert_eq!(
                parse_digest(&value).expect(&value),
                digest,
                "value {value:?}"
            );
        }
        // 形态不对：空 / 只有前缀 / 太短 / 非 hex / 别的 scheme。
        for value in [
            "",
            "sha256:",
            "deadbeef",
            &digest[..63],
            "zzzz",
            "sha512:abc",
        ] {
            assert!(
                parse_digest(value).is_err(),
                "value {value:?} should be rejected"
            );
        }
    }
}
