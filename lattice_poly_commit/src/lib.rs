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
