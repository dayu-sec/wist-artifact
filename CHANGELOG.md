# 更新日志

本文件记录 `wist-artifact` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.0] - 2026-10-06

首个版本：把「制品的传输与校验」从 `wist-release` 里分出来单独一层。

### 为什么分

这几件事**控制面与被管端都要做**，但它们不该背上对方的域概念：被管端（agentd）只需要「把那份包
拿下来、核一下摘要、判一下版本方向」；控制面在此之上才做**发布计划**与**包身份解析**。
分出来之后 agentd 不必因为「发布域」升一版就被迫跟版，也不会凭空背上解包依赖（`flate2` / `tar`）。

### 新增

- **`source`**：取来源 —— `/绝对路径` 或 URL；上限与超时是**调用方的策略**（`read_source_within`），
  也可带**调用方自己的 client**（`read_source_with_client`，agentd 的 mTLS 身份靠它）；
  只要本机路径那一支用 `read_local_source`。超限是单独一类错误（`ArtifactError::TooLarge`）。
  另有 `read_verified_source`（读 + 校验期望摘要）。
- **`digest`**：`sha256_hex_bytes`（裸 hex 小写）、`parse_digest`（可选 `sha256:` 前缀、大小写不敏感、
  必须 64 位 hex）。摘要用 **`sha2`**（纯 Rust）而不是 `ring` —— 不把 C/汇编依赖带给每个消费方，
  与 `wist-contracts::fact_summary` 选 `sha2` 的理由一致。
- **`version`**：`normalize_version`（只剥一个 `v`/`V` 前缀）、`parse_version`、
  `version_is_newer`（数字段逐段比，`0.10 > 0.9`；认不出来返回 `None`，让调用方拒绝而不是猜）。
- **`naming`**：`content_id(prefix, sha)`、`package_id_for_sha256`、`is_safe_path_segment`
  （组件名 / 版本号 / 文件名必须真的只有一段）、`artifact_filename`（返回值保证是安全路径段）。
