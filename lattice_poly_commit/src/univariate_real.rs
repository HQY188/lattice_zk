//! **真实**单变量（系数向量）承诺：把次数 `< N` 的多项式系数用 `encode_block` 编成单个环消息块，再 **`AjtaiCrs::commit`**。
//!
//! - **`UniCommitmentReal`**：`blocks` 一般为一块 Ajtai 承诺（原型 `ell=1`）。
//! - **`UniOpeningReal`**：明文消息块 `m_blocks` + 对应的 Ajtai 随机性 `r_blocks`（每块是长度 `nu` 的环多项式向量）。
//! - **`open_real`**：检查 `m` 是否等于系数编码、范数是否低于 `beta_open`、承诺是否等于 `commit(m,r)`。
//! - **`open_real_commitment_only`**：MLE 最后一步：\(m_z = m_a + e m_u\) 不再对应简单“系数嵌入”，只验证 **承诺与打开一致**。
//! - **`eval_real` / `verify_real`**：在基域上按系数求值（系数从第一条 RNS 腿读出）；原型揭示 `m`，非 succinct。

use arith::Field;
use rand::RngCore;
use serdes::{ExpSerde, SerdeResult};

use crate::{
    ajtai::{AjtaiCommitment, AjtaiCrs},
    encoder::encode_block,
    params::LatticeParams,
    ring::PolyRns,
};

/// Univariate commitment key (real Ajtai-based).
#[derive(Clone, Debug)]
pub struct UniCkReal {
    pub params: LatticeParams,
    pub crs: AjtaiCrs,
}

impl ExpSerde for UniCkReal {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.params.serialize_into(&mut writer)?;
        self.crs.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let params = LatticeParams::deserialize_from(&mut reader)?;
        let crs = AjtaiCrs::deserialize_from(&mut reader)?;
        Ok(Self { params, crs })
    }
}

/// Commitment: a vector of Ajtai commitments for each block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniCommitmentReal {
    pub blocks: Vec<AjtaiCommitment>,
}

/// Opening info δ = (m_blocks, r_blocks).
#[derive(Clone, Debug, Default)]
pub struct UniOpeningReal {
    pub m_blocks: Vec<PolyRns>,
    pub r_blocks: Vec<Vec<PolyRns>>,
}

impl ExpSerde for UniCommitmentReal {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.blocks.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let blocks = Vec::<AjtaiCommitment>::deserialize_from(&mut reader)?;
        Ok(Self { blocks })
    }
}

impl ExpSerde for UniOpeningReal {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.m_blocks.serialize_into(&mut writer)?;
        self.r_blocks.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let m_blocks = Vec::<PolyRns>::deserialize_from(&mut reader)?;
        let r_blocks = Vec::<Vec<PolyRns>>::deserialize_from(&mut reader)?;
        Ok(Self { m_blocks, r_blocks })
    }
}

/// Setup(1^λ, N) -> ck. (λ ignored in this prototype; CRS sampled from rng)
pub fn setup_real(lambda: usize, n: usize, rng: &mut impl RngCore) -> UniCkReal {
    let _ = lambda;
    // Grow the negacyclic ring so one block can hold all row coefficients (MLE column count).
    let mut params = LatticeParams::for_univariate_coeffs(n);
    params.ell = 1;
    assert!(n <= params.ring_degree, "N must fit in one block");
    let crs = AjtaiCrs::setup(params.clone(), rng);
    UniCkReal { params, crs }
}

/// Commit to coefficients h[0..N).
pub fn commit_real<F: Field>(
    ck: &UniCkReal,
    coeffs: &[F],
    rng: &mut impl RngCore,
) -> (UniCommitmentReal, UniOpeningReal) {
    let r_vec = ck.crs.sample_rand_vec(rng);
    commit_real_with_r_vec(ck, coeffs, r_vec)
}

/// Same as [`commit_real`], but uses pre-sampled randomness (e.g. after sequential RNG draws).
/// Enables row-parallel multilinear commit while preserving the same RNG stream as sequential `commit_real`.
pub fn commit_real_with_r_vec<F: Field>(
    ck: &UniCkReal,
    coeffs: &[F],
    r_vec: Vec<PolyRns>,
) -> (UniCommitmentReal, UniOpeningReal) {
    assert!(coeffs.len() <= ck.params.ring_degree);
    let m = encode_block(coeffs, &ck.params);
    let c = ck.crs.commit(&[m.clone()], &r_vec);
    (
        UniCommitmentReal { blocks: vec![c] },
        UniOpeningReal {
            m_blocks: vec![m],
            r_blocks: vec![r_vec],
        },
    )
}

/// Open: check that commitment equals Ajtai commit of opening.
pub fn open_real<F: Field>(
    ck: &UniCkReal,
    com: &UniCommitmentReal,
    coeffs: &[F],
    delta: &UniOpeningReal,
) -> bool {
    let debug = std::env::var("LATTICE_MLE_DEBUG").is_ok();
    if com.blocks.len() != 1 || delta.m_blocks.len() != 1 || delta.r_blocks.len() != 1 {
        if debug {
            eprintln!("[uni_open] fail: shape mismatch");
        }
        return false;
    }
    let m_expected = encode_block(coeffs, &ck.params);
    if delta.m_blocks[0] != m_expected {
        if debug {
            eprintln!("[uni_open] fail: message mismatch");
        }
        return false;
    }
    // Norm bounds (prototype).
    let mut norm = delta.m_blocks[0].norm_sq_first_modulus();
    for r in &delta.r_blocks[0] {
        norm += r.norm_sq_first_modulus();
    }
    if norm.sqrt() > ck.params.beta_open {
        if debug {
            eprintln!(
                "[uni_open] fail: norm bound (sqrt(norm)={} > beta_open={})",
                norm.sqrt(),
                ck.params.beta_open
            );
        }
        return false;
    }
    let c_expected = ck.crs.commit(&[delta.m_blocks[0].clone()], &delta.r_blocks[0]);
    let ok = com.blocks[0] == c_expected;
    if !ok && debug {
        eprintln!("[uni_open] fail: commitment mismatch");
    }
    ok
}

/// Open (commitment-only): check that commitment equals Ajtai commit of (m, r) and norm bound.
/// Does not check that m equals encode(coeffs); use for MLE z-opening when m = m_a + e*m_u (ring).
pub fn open_real_commitment_only(
    ck: &UniCkReal,
    com: &UniCommitmentReal,
    delta: &UniOpeningReal,
) -> bool {
    let debug = std::env::var("LATTICE_MLE_DEBUG").is_ok();
    if com.blocks.len() != 1 || delta.m_blocks.len() != 1 || delta.r_blocks.len() != 1 {
        if debug {
            eprintln!("[uni_open] fail: shape mismatch");
        }
        return false;
    }
    let mut norm = delta.m_blocks[0].norm_sq_first_modulus();
    for r in &delta.r_blocks[0] {
        norm += r.norm_sq_first_modulus();
    }
    if norm.sqrt() > ck.params.beta_open {
        if debug {
            eprintln!(
                "[uni_open] fail: norm bound (sqrt(norm)={} > beta_open={})",
                norm.sqrt(),
                ck.params.beta_open
            );
        }
        return false;
    }
    let c_expected = ck.crs.commit(&[delta.m_blocks[0].clone()], &delta.r_blocks[0]);
    let ok = com.blocks[0] == c_expected;
    if !ok && debug {
        eprintln!("[uni_open] fail: commitment mismatch");
    }
    ok
}

/// Eval(x, δ) -> (y, ρ). Prototype: reveal encoded polynomial and randomness as proof.
pub fn eval_real<F: Field>(x: &F, delta: &UniOpeningReal) -> (F, UniOpeningReal) {
    // Decode m (coeff domain) from first modulus only and evaluate in F.
    // Since our encoding is direct coefficient embedding, we can evaluate using the original
    // coefficients stored in delta by reinterpreting coefficient layer as u64 and mapping to F.
    let coeffs_u64 = &delta.m_blocks[0].coeffs[0];
    let mut y = F::ZERO;
    let mut pow = F::ONE;
    for &c in coeffs_u64 {
        y += F::from_u256(ethnum::U256::from(c as u128)) * pow;
        pow *= *x;
    }
    (y, delta.clone())
}

/// Verify(ck, c, x, y, ρ).
pub fn verify_real<F: Field>(
    ck: &UniCkReal,
    com: &UniCommitmentReal,
    x: &F,
    y: F,
    rho: &UniOpeningReal,
) -> bool {
    let (y2, _) = eval_real(x, rho);
    if y2 != y {
        return false;
    }
    // In this prototype, rho contains m, so we can open without separately providing coeffs.
    let coeffs_u64 = &rho.m_blocks[0].coeffs[0];
    let coeffs_f: Vec<F> = coeffs_u64
        .iter()
        .map(|&c| F::from_u256(ethnum::U256::from(c as u128)))
        .collect();
    open_real::<F>(ck, com, &coeffs_f, rho)
}

