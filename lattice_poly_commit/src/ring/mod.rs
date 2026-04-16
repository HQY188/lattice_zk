//! 环算术子模块：在 RNS 下表示 \( R_q = \mathbb{Z}_q[X]/(X^n+1) \)。
//!
//! - **`poly`**：`PolyRns`（多层 CRT）、加/减、否定循环乘法、打开时的范数估计。
//! - **`ntt`**：每个素模数一张否定循环 NTT 表，`NttPlan` 聚合多条 RNS 腿。

mod ntt;
mod poly;

pub use ntt::{NttPlan, NttTable};
pub use poly::{PolyRns, RnsModuli};

