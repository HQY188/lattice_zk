//! 基于格的多元（MLE）多项式承诺原型实现。
//!
//! ## 与 `mle_pc.md` 的对应关系（便于核对代数）
//!
//! - **`params`**：环维度、RNS 模数、Ajtai 维度 `mu, nu, ell`（`A0` 为 mu×ell、`A1` 为 mu×nu）、消息块大小 `n`、噪声与范数界。
//! - **`ring`**：`PolyRns` 表示 \( \mathbb{Z}_q[X]/(X^n+1) \)，用否定循环 NTT 做环上乘法（承诺里 `A·m` 的 `*` 即此乘法）。
//! - **`ajtai`**：CRS `A0,A1` 与承诺 `Com(m,r)=A0·m+A1·r`（按行/按 μ 累加）。
//! - **`encoder`**：`Ecd`：把基域系数向量压进一个环多项式（原型：前几项直接嵌入，与标量乘一致）。
//! - **`sampler`**：离散高斯等，用于采样 `r`（Ajtai 随机性向量）。
//! - **`univariate`**：仅占位的“系数即承诺”接口，供与论文符号对齐；**真实**行承诺在 **`univariate_real`**。
//! - **`multilinear`**：由超立方体取值 `f` 构造矩阵 `T`、对每一行做 `UniCommitmentReal`、计算 `u=A·T`、`y=<u,B>` 及 Fiat-Shamir 所需的 `z` 打开。
//! - **`expander_api`**：实现 `gkr_engine::ExpanderPCS`，把上述流程接到 Expander/GKR（含 SIMD/MPI 下的超立方体重排）。
//!
//! Rust 提示：`pub use` 把子模块类型重新导出到 crate 根，外部可用 `lattice_poly_commit::MleCk` 等路径引用。

#![allow(clippy::manual_div_ceil)]

mod expander_api;
pub mod multilinear;
mod univariate;
mod ring;
mod params;
mod sampler;
mod ajtai;
mod encoder;
mod univariate_real;

pub use expander_api::{LatticeMleOpening, LatticeMlePCS, LatticeMleScratchPad};
pub use multilinear::{
    build_matrix_t, MleCk, MleCommitment, MleOpening, ENV_COMMIT_PARALLEL, ENV_COMMIT_THREADS, IOTA,
    PAR_DOT_PRODUCT_MIN_LEN,
};
pub use univariate::{UniCk, UniCommitment, UniOpening};
pub use ring::{NttPlan, NttTable, PolyRns, RnsModuli};
pub use params::LatticeParams;
pub use ajtai::{
    AjtaiCommitment, AjtaiCrs, ENV_AJTAI_COMMIT_PARALLEL, PAR_AJTAI_COMMIT_MIN_MU,
};
pub use encoder::{decode_block, encode_block};
pub use univariate_real::{commit_real, open_real, setup_real, UniCkReal, UniCommitmentReal, UniOpeningReal, verify_real, eval_real};
