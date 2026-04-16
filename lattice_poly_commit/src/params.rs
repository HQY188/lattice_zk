//! 全库共享的格承诺参数：环、模数、Ajtai 形状、编码块大小与（原型）统计/范数界。
//!
//! `LatticeParams::default_small()` 给出一组可在 MSVC 上跑的默认：Goldilocks 素数、256 维否定循环环。

use crate::ring::{NttPlan, RnsModuli};
use serdes::{ExpSerde, SerdeResult};

/// 基于环 \(R_q\) 的单变量/多元子承诺所共用的参数（原型）。
///
/// This implementation is **MSVC-friendly** (pure Rust) and focuses on correctness of the
/// algebraic relations used by `mle_pc.md`. It intentionally keeps the encoding simple
/// (direct coefficient embedding) to avoid big-integer / MPFR dependencies.
#[derive(Clone, Debug)]
pub struct LatticeParams {
    /// Ring degree d (power of two). We work in R_q = Z_q[X]/(X^d + 1).
    pub ring_degree: usize,
    /// CRT primes (RNS).
    pub moduli: RnsModuli,
    /// psi[i] is a primitive 2d-th root of unity mod moduli[i] (for negacyclic NTT).
    pub psi: Vec<u64>,

    /// Ajtai dimensions: A0 in R_q^{mu x ell}, A1 in R_q^{mu x nu}.
    pub mu: usize,
    pub nu: usize,
    pub ell: usize,

    /// Coefficient chunk size n (we commit to blocks of size n, with n<=ell).
    pub n: usize,

    /// Standard deviation for randomness (prototype).
    pub sigma: f64,

    /// Norm bounds (prototype).
    pub beta_open: f64,
    pub beta_eval: f64,
}

impl LatticeParams {
    pub fn ntt_plan(&self) -> NttPlan {
        NttPlan::new(self.ring_degree, self.moduli.clone(), self.psi.clone())
    }

    /// A small, MSVC-friendly default parameter set.
    ///
    /// - ring_degree = 256
    /// - modulus = Goldilocks prime (2^64 - 2^32 + 1)
    /// - mu=1, nu=2, ell=1, n=256
    pub fn default_small() -> Self {
        Self::for_univariate_coeffs(256)
    }

    /// Parameters for committing up to `num_coeffs` coefficients in one negacyclic block.
    ///
    /// `ring_degree` is the smallest power of two ≥ max(256, `num_coeffs`), so MLE rows with
    /// `n_cols` up to that size are supported (see [`crate::multilinear::setup`]).
    pub fn for_univariate_coeffs(num_coeffs: usize) -> Self {
        let ring_degree = num_coeffs.next_power_of_two().max(256);
        let moduli = RnsModuli::new(vec![goldilocks::GOLDILOCKS_MOD]);
        let psi = vec![goldilocks_psi(ring_degree)];
        Self {
            ring_degree,
            moduli,
            psi,
            mu: 1,
            nu: 2,
            ell: 1,
            n: ring_degree,
            sigma: 3.2,
            beta_open: 1e30,
            beta_eval: 1e30,
        }
    }
}

impl ExpSerde for LatticeParams {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.ring_degree.serialize_into(&mut writer)?;
        self.moduli.serialize_into(&mut writer)?;
        self.psi.serialize_into(&mut writer)?;
        self.mu.serialize_into(&mut writer)?;
        self.nu.serialize_into(&mut writer)?;
        self.ell.serialize_into(&mut writer)?;
        self.n.serialize_into(&mut writer)?;
        self.sigma.serialize_into(&mut writer)?;
        self.beta_open.serialize_into(&mut writer)?;
        self.beta_eval.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let ring_degree = usize::deserialize_from(&mut reader)?;
        let moduli = RnsModuli::deserialize_from(&mut reader)?;
        let psi = Vec::<u64>::deserialize_from(&mut reader)?;
        let mu = usize::deserialize_from(&mut reader)?;
        let nu = usize::deserialize_from(&mut reader)?;
        let ell = usize::deserialize_from(&mut reader)?;
        let n = usize::deserialize_from(&mut reader)?;
        let sigma = f64::deserialize_from(&mut reader)?;
        let beta_open = f64::deserialize_from(&mut reader)?;
        let beta_eval = f64::deserialize_from(&mut reader)?;
        Ok(Self {
            ring_degree,
            moduli,
            psi,
            mu,
            nu,
            ell,
            n,
            sigma,
            beta_open,
            beta_eval,
        })
    }
}

/// 二进制快速幂：\(a^e \bmod q\)，供构造本原根幂次使用。
fn pow_mod(mut a: u64, mut e: u64, q: u64) -> u64 {
    let mut r = 1u64;
    while e > 0 {
        if e & 1 == 1 {
            r = ((r as u128 * a as u128) % (q as u128)) as u64;
        }
        a = ((a as u128 * a as u128) % (q as u128)) as u64;
        e >>= 1;
    }
    r
}

/// 对 Goldilocks 模数，从内置 2^32 阶单位根推出长度 `2d` 否定循环 NTT 所需的 `psi`（满足 \(\psi^d=-1\)等）。
fn goldilocks_psi(d: usize) -> u64 {
    use arith::FFTField;
    // Goldilocks has a primitive 2^32 root in FFTField::root_of_unity().
    let q = goldilocks::GOLDILOCKS_MOD;
    let root_2_32 = goldilocks::Goldilocks::root_of_unity().v;
    let k_plus_1 = (2 * d).ilog2() as u64;
    assert!(k_plus_1 <= 32);
    let shift = 32 - k_plus_1;
    pow_mod(root_2_32, 1u64 << shift, q)
}

