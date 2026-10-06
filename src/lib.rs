//! 制品的**传输与校验**：读来源、限大小、算/校验摘要、版本比较、命名与寻址。
//!
//! 只管「一个制品怎么被拿到、怎么被验证、怎么被命名」：
//!
//! - **不含发布计划**（阶段切分 / 推进闸门 / 灰度节流）—— 在 `wist-release` 的 `rollout`；
//! - **不含包身份解析**（`<name>-<version>-<triple>` 顶层目录名那套布局口径）—— 在
//!   `wist-release` 的 `package`。
//!
//! 这样**被管端**（`wist-agentd`）只需要这一层，不必背上控制面的发布域概念，也不会凭空多出解包依赖
//! （`flate2` / `tar` 只在身份解析那层用）。控制面（center / gateway / gwlinkd）在此之上再叠发布域。

pub mod digest;
pub mod naming;
pub mod source;
pub mod version;
