//! 取来源：`/绝对路径` 或 URL → 字节，带**大小上限**与**超时**。
//!
//! 机制（甄别路径 / URL、读完前拦超限、错误分类）只有这一份；**策略**（多大算大、等多久、
//! 用哪个 HTTP client）由调用方给 —— 例如 agentd 的取包要走它自己的 **mTLS 客户端证书**，
//! 中心拉 GitHub 制品走自己的默认 client。

use std::time::Duration;

use crate::digest::{parse_digest, sha256_hex_bytes};

/// 拉取来源的默认超时（制品可能几十 MB，给足时间，但不能无限等）。
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(120);
/// 单个制品的默认大小上限（防误拉一个大文件把内存吃光）。
pub const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

/// 取制品失败：调用方据此区分「摘要填错了」「来源拿不到」「来源太大」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    /// 来源读不到（路径不存在 / URL 拉不到 / HTTP 非 2xx）。
    SourceUnavailable(String),
    /// 与期望摘要不符。
    DigestMismatch(String),
    /// 来源**超过大小上限**（读完前就拦下）——单独一类，方便调用方给出「太大了」这种可操作的回应。
    TooLarge(String),
}

impl std::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactError::SourceUnavailable(detail)
            | ArtifactError::DigestMismatch(detail)
            | ArtifactError::TooLarge(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for ArtifactError {}

/// 读来源（**本机绝对路径** 或 URL）→ 字节，用默认上限与超时。
///
/// 甄别方式很简单：**以 `/` 开头**当本机绝对路径，其余一律当 URL 交给 reqwest。于是：
/// - 相对路径（`pkgs/x.tar.gz`）不算「路径」，会被当成 URL 解析并失败 —— 本 crate 只认绝对路径；
/// - **不限制 scheme**（`http://` 也收）：是否强制 https 属于调用方的策略，内容真伪由
///   `expected_sha256` 兜底（见 [`read_verified_source`]）。
///
/// 「本机路径」是允许且常见的：制品可能就在本机、外网访问不到，或还在开发。
pub async fn read_source(source: &str) -> Result<Vec<u8>, ArtifactError> {
    read_source_within(source, MAX_ARTIFACT_BYTES, FETCH_TIMEOUT).await
}

/// [`read_source`] 的通用形态：**大小上限与超时由调用方给** —— 机制共用一份，策略各域自己定：
///
/// - 安装包：512 MiB / 120s（[`MAX_ARTIFACT_BYTES`] / [`FETCH_TIMEOUT`]，即 [`read_source`]）；
/// - 知识库内容包小得多：16 MiB / 60s。
///
/// 上限对**两条来源都生效**，且都在**读完前**先拦一道：
/// - 本机路径：先看 `metadata().len()`，超限就直接拒，不把大文件读进内存；
/// - http：先看 `Content-Length`（响应头可能不给，所以读回后再核一次实际长度）。
///
/// ⚠️ 本机分支是**同步** `std::fs::read`（本 crate 不引 tokio 运行时）：从 async 上下文调用时，
/// 读一个几百 MB 的本地文件会占住当前 worker —— 介意就由调用方套 `spawn_blocking`。
pub async fn read_source_within(
    source: &str,
    max_bytes: u64,
    timeout: Duration,
) -> Result<Vec<u8>, ArtifactError> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| {
            ArtifactError::SourceUnavailable(format!("failed to build http client: {err}"))
        })?;
    read_source_with_client(&client, source, max_bytes, timeout).await
}

/// 与 [`read_source_within`] 同，但用**调用方给的 client**：例如 agentd 取包要走它自己的
/// **mTLS 客户端证书**（网关的分发端点按 agent 身份认证）；本函数不替它构造 client，就不丢掉那层身份。
///
/// `timeout` 作用在**本次请求**上（调用方给的 client 可能没设超时，或是为别的用途配的）。
pub async fn read_source_with_client(
    client: &reqwest::Client,
    source: &str,
    max_bytes: u64,
    timeout: Duration,
) -> Result<Vec<u8>, ArtifactError> {
    if source.starts_with('/') {
        return read_local_source(source, max_bytes);
    }
    let response = client
        .get(source)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| {
            ArtifactError::SourceUnavailable(format!(
                "failed to fetch artifact from {source}: {err}"
            ))
        })?;
    if !response.status().is_success() {
        return Err(ArtifactError::SourceUnavailable(format!(
            "artifact source {source} returned HTTP {}",
            response.status()
        )));
    }
    if let Some(len) = response.content_length()
        && len > max_bytes
    {
        return Err(over_limit(source, len, max_bytes));
    }
    let bytes = response.bytes().await.map_err(|err| {
        ArtifactError::SourceUnavailable(format!(
            "failed to read artifact body from {source}: {err}"
        ))
    })?;
    if bytes.len() as u64 > max_bytes {
        return Err(over_limit(source, bytes.len() as u64, max_bytes));
    }
    Ok(bytes.to_vec())
}

/// 读本机路径：先按元数据拦超限，再读，读完再核一次实际长度。
///
/// 两道检查都是必要的：元数据那道保证**不把超大文件读进内存**；实际长度那道挡住
/// 「看元数据时还没长大」的空档（元数据与读之间文件可能变）。
///
/// 单拎出来是因为它有单独的用途：调用方想先自己决定「这次是本机路径还是远端」（例如本机路径
/// 不该去构造一个需要身份 / 信任锚的 HTTP client）时，可以直接走这一支。
pub fn read_local_source(path: &str, max_bytes: u64) -> Result<Vec<u8>, ArtifactError> {
    let metadata = std::fs::metadata(path).map_err(|err| {
        ArtifactError::SourceUnavailable(format!("failed to read artifact from {path}: {err}"))
    })?;
    if metadata.len() > max_bytes {
        return Err(over_limit(path, metadata.len(), max_bytes));
    }
    let bytes = std::fs::read(path).map_err(|err| {
        ArtifactError::SourceUnavailable(format!("failed to read artifact from {path}: {err}"))
    })?;
    if bytes.len() as u64 > max_bytes {
        return Err(over_limit(path, bytes.len() as u64, max_bytes));
    }
    Ok(bytes)
}

fn over_limit(source: &str, actual: u64, max_bytes: u64) -> ArtifactError {
    ArtifactError::TooLarge(format!(
        "artifact at {source} is {actual} bytes, over the {max_bytes} byte limit"
    ))
}

/// 读来源 → 校验期望摘要，返回（字节, 裸 hex sha256）。
///
/// 期望摘要先过 [`parse_digest`]（前缀 / 大小写 / 长度都在那里收口）——
/// **填错的摘要**因此会报「摘要形态不对」而不是等成一次不相符。
pub async fn read_verified_source(
    source: &str,
    expected_sha256: Option<&str>,
) -> Result<(Vec<u8>, String), ArtifactError> {
    let bytes = read_source(source).await?;
    let actual = sha256_hex_bytes(&bytes);
    if let Some(expected) = expected_sha256 {
        let expected_hex = parse_digest(expected).map_err(|detail| {
            ArtifactError::DigestMismatch(format!("expected sha256 is malformed: {detail}"))
        })?;
        if actual != expected_hex {
            return Err(ArtifactError::DigestMismatch(format!(
                "artifact sha256 mismatch: expected {expected_hex} got {actual}"
            )));
        }
    }
    Ok((bytes, actual))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_a_local_path_and_verifies_the_expected_digest() {
        let path = std::env::temp_dir().join("wist-artifact-src.bin");
        std::fs::write(&path, b"payload").expect("write");
        let source = path.to_string_lossy().to_string();

        let (bytes, sha) = read_verified_source(&source, None).await.expect("read");
        assert_eq!(bytes, b"payload");
        assert_eq!(sha.len(), 64);

        read_verified_source(&source, Some(&sha))
            .await
            .expect("matching digest");
        read_verified_source(&source, Some(&format!("sha256:{sha}")))
            .await
            .expect("prefixed digest");
        let err = read_verified_source(&source, Some("deadbeef"))
            .await
            .expect_err("mismatch");
        assert!(matches!(err, ArtifactError::DigestMismatch(_)), "{err}");

        let err = read_source("/definitely/not/here.bin")
            .await
            .expect_err("missing");
        assert!(matches!(err, ArtifactError::SourceUnavailable(_)), "{err}");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn read_verified_source_accepts_a_prefixed_digest_in_any_case() {
        let path = std::env::temp_dir().join("wist-artifact-digest-prefix.bin");
        std::fs::write(&path, b"payload").expect("write");
        let source = path.to_string_lossy().to_string();
        let digest = sha256_hex_bytes(b"payload");
        let upper = digest.to_ascii_uppercase();

        for expected in [
            format!("sha256:{digest}"),
            format!("SHA256:{digest}"),
            format!(" sha256: {upper} "),
            upper.clone(),
        ] {
            read_verified_source(&source, Some(&expected))
                .await
                .unwrap_or_else(|err| panic!("expected {expected} to verify: {err}"));
        }
        // 空串 / 只有前缀 / 截断的摘要 → 不符（不静默放行）。
        for bad in ["", "sha256:", &digest[..8]] {
            assert!(
                matches!(
                    read_verified_source(&source, Some(bad)).await,
                    Err(ArtifactError::DigestMismatch(_))
                ),
                "expected {bad:?} to be rejected"
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn read_source_within_caps_local_files_before_reading_them() {
        let path = std::env::temp_dir().join("wist-artifact-size-cap.bin");
        std::fs::write(&path, b"0123456789").expect("write"); // 10 字节
        let source = path.to_string_lossy().to_string();

        // 上限 8：超限就拒，且错误里说清是「超了多少字节 / 上限多少」。
        let err = read_source_within(&source, 8, FETCH_TIMEOUT)
            .await
            .expect_err("over limit");
        // 超限是**单独一类**（调用方据此回一句「太大了」，而不是笼统的「拿不到」）。
        assert!(matches!(err, ArtifactError::TooLarge(_)), "{err}");
        assert!(err.to_string().contains("over the 8 byte limit"), "{err}");
        // 恰好等于上限：放行。
        assert_eq!(
            read_source_within(&source, 10, FETCH_TIMEOUT)
                .await
                .expect("at limit"),
            b"0123456789"
        );
        // 目录：读不了，但不 panic。
        let err = read_source_within(
            &std::env::temp_dir().to_string_lossy(),
            1 << 40,
            FETCH_TIMEOUT,
        )
        .await
        .expect_err("directory");
        assert!(matches!(err, ArtifactError::SourceUnavailable(_)));
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn read_source_with_client_still_reads_local_paths() {
        // 本机路径分支不看 client（agentd 取包也走这条），但入口要能一起用。
        let path = std::env::temp_dir().join("wist-artifact-with-client.bin");
        std::fs::write(&path, b"payload").expect("write");
        let client = reqwest::Client::new();
        assert_eq!(
            read_source_with_client(&client, &path.to_string_lossy(), 1024, FETCH_TIMEOUT)
                .await
                .expect("read"),
            b"payload"
        );
        assert!(matches!(
            read_source_with_client(&client, "/definitely/not/here.bin", 1024, FETCH_TIMEOUT).await,
            Err(ArtifactError::SourceUnavailable(_))
        ));
        let _ = std::fs::remove_file(path);
    }
}
