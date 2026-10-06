# wist-artifact

`wist` 制品的**传输与校验**口径：一个制品怎么被拿到、怎么被验证、怎么被命名。只此一份。

[![crates.io](https://img.shields.io/crates/v/wist-artifact.svg)](https://crates.io/crates/wist-artifact)
[![docs.rs](https://img.shields.io/docsrs/wist-artifact/latest.svg)](https://docs.rs/wist-artifact)
[![CI](https://github.com/dayu-sec/wist-artifact/actions/workflows/ci.yml/badge.svg)](https://github.com/dayu-sec/wist-artifact/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

## 为什么单独一层

「读来源 / 算摘要 / 比版本 / 命名」这几件事，**控制面与被管端都要做**，但它们不该背上对方的域概念：

- 被管端（`wist-agentd`）只需要「把那份包拿下来、核一下摘要、判一下版本方向」；
- 控制面（`wist-center` / `wist-gateway`）在此之上还要做**发布计划**与**包身份解析**。

后者在 [`wist-release`](../wist-release)，前者在这一层。分开之后 agentd 不必因为「发布域」升一版
就被迫跟版，也不必凭空背上解包依赖（`flate2` / `tar`）。

## 模块

| 模块      | 内容                                                                 |
| --------- | -------------------------------------------------------------------- |
| `source`  | 取来源：`/绝对路径` 或 URL（可注入调用方的 client，例如 agentd 的 mTLS）、大小上限、`ArtifactError` |
| `digest`  | `sha256_hex_bytes`（裸 hex）、`parse_digest`（`sha256:` 前缀 / 大小写 / 64 位 hex 校验） |
| `version` | `normalize_version`、`parse_version`、`version_is_newer`（数值逐段比，认不出返回 `None`） |
| `naming`  | `is_safe_path_segment`（路径段安全）、`artifact_filename`、`content_id` |

## 消费方

- [`wist-agentd`](../wist-agentd) —— 升级时取包与校验。
- [`wist-release`](../wist-release) —— 发布域（包身份解析 + 发布计划）以本 crate 为底座，并 re-export 它，
  让 center / gateway 的调用点不必改。
- [`wist-center`](../wist-center) / [`wist-gateway`](../wist-gateway) —— 经 `wist-release` 使用。

## License

[Apache-2.0](LICENSE)
