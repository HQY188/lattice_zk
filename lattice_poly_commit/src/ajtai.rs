//! Ajtai 型线性承诺：\( \mathbf{c} = A_0 \mathbf{m} + A_1 \mathbf{r} \)，其中每个条目是环 \(R_q\) 上元素，
//! 环乘用否定循环卷积（见 `PolyRns::mul_negacyclic`）。
//!
//! - **`AjtaiCrs::setup`**：均匀随机采样矩阵多项式。
//! - **`commit`**：对消息向量 `m`（长度 `ell`）与随机性 `r`（长度 `nu`）按行累加；可选并行 μ 维。
//! - **`sample_rand_vec`**：按离散高斯采样 `r`（原型实现，非密码学硬化）。

use rand::RngCore;
use rayon::prelude::*;
use serdes::{ExpSerde, SerdeResult};

use crate::{
    params::LatticeParams,
    ring::{PolyRns, RnsModuli},
    sampler::GaussianSampler,
};

/// Set to `0`, `false`, or `no` (case-insensitive) to force sequential μ-loop in [`AjtaiCrs::commit`].
/// Default: parallel when [`PAR_AJTAI_COMMIT_MIN_MU`] is met and the caller is **not** already on a Rayon worker
/// (so MLE row-parallel commit does not nest another `par_iter` inside each row).
pub const ENV_AJTAI_COMMIT_PARALLEL: &str = "LATTICE_AJTAI_COMMIT_PARALLEL";

/// Minimum μ to use parallel [`AjtaiCrs::commit`] (avoids Rayon overhead on tiny matrices).
pub const PAR_AJTAI_COMMIT_MIN_MU: usize = 4;

fn ajtai_commit_parallel_enabled() -> bool {
    match std::env::var(ENV_AJTAI_COMMIT_PARALLEL) {
        Ok(s) => {
            let s = s.trim().to_ascii_lowercase();
            !(s.is_empty() || s == "0" || s == "false" || s == "no")
        }
        Err(_) => true,
    }
}

#[inline]
fn should_parallelize_ajtai_commit(mu: usize) -> bool {
    ajtai_commit_parallel_enabled()
        && mu >= PAR_AJTAI_COMMIT_MIN_MU
        && rayon::current_thread_index().is_none()
}

/// Ajtai CRS: A0 in R_q^{mu x ell}, A1 in R_q^{mu x nu}.
#[derive(Clone, Debug)]
pub struct AjtaiCrs {
    pub params: LatticeParams,
    pub a0: Vec<Vec<PolyRns>>,
    pub a1: Vec<Vec<PolyRns>>,
}

/// Ajtai commitment: vector length mu of ring elements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AjtaiCommitment {
    pub value: Vec<PolyRns>,
}

impl AjtaiCommitment {
    pub fn zero(mu: usize, ring_degree: usize, moduli: RnsModuli) -> Self {
        Self {
            value: (0..mu).map(|_| PolyRns::zero(ring_degree, moduli.clone())).collect(),
        }
    }

    pub fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(self.value.len(), rhs.value.len());
        for (a, b) in self.value.iter_mut().zip(rhs.value.iter()) {
            a.add_assign(b);
        }
    }
}

impl AjtaiCrs {
    pub fn setup(params: LatticeParams, rng: &mut impl RngCore) -> Self {
        let a0 = sample_uniform_matrix(
            params.mu,
            params.ell,
            params.ring_degree,
            &params.moduli,
            rng,
        );
        let a1 = sample_uniform_matrix(
            params.mu,
            params.nu,
            params.ring_degree,
            &params.moduli,
            rng,
        );
        Self {
            params,
            a0,
            a1,
        }
    }

    /// Commit to message vector `m` (length ell) with randomness vector `r` (length nu).
    ///
    /// When μ ≥ [`PAR_AJTAI_COMMIT_MIN_MU`], [`ENV_AJTAI_COMMIT_PARALLEL`] allows it, and the caller is not
    /// already on a Rayon worker thread, the outer μ loop is parallelized (shared read-only NTT plan and CRS).
    pub fn commit(&self, m: &[PolyRns], r: &[PolyRns]) -> AjtaiCommitment {
        assert_eq!(m.len(), self.params.ell);
        assert_eq!(r.len(), self.params.nu);
        let mu = self.params.mu;
        if should_parallelize_ajtai_commit(mu) {
            let ntt = self.params.ntt_plan();
            let ring_degree = self.params.ring_degree;
            let moduli = self.params.moduli.clone();
            let ell = self.params.ell;
            let nu = self.params.nu;
            let a0 = &self.a0;
            let a1 = &self.a1;
            let value: Vec<PolyRns> = (0..mu)
                .into_par_iter()
                .map(|i| {
                    // 与顺序版本相同：第 i 行与 m、r 的环上双线性形式
                    let mut acc = PolyRns::zero(ring_degree, moduli.clone());
                    for j in 0..ell {
                        let prod = a0[i][j].mul_negacyclic(&m[j], &ntt);
                        acc.add_assign(&prod);
                    }
                    for j in 0..nu {
                        let prod = a1[i][j].mul_negacyclic(&r[j], &ntt);
                        acc.add_assign(&prod);
                    }
                    acc
                })
                .collect();
            return AjtaiCommitment { value };
        }

        let mut out = AjtaiCommitment::zero(mu, self.params.ring_degree, self.params.moduli.clone());
        let ntt = self.params.ntt_plan();

        for i in 0..mu {
            // 第 i 个承诺分量：环上 c_i = Σ_j A0[i,j]*m[j] + Σ_j A1[i,j]*r[j]
            for j in 0..self.params.ell {
                let prod = self.a0[i][j].mul_negacyclic(&m[j], &ntt);
                out.value[i].add_assign(&prod);
            }
            for j in 0..self.params.nu {
                let prod = self.a1[i][j].mul_negacyclic(&r[j], &ntt);
                out.value[i].add_assign(&prod);
            }
        }
        out
    }

    /// Sample a randomness vector of length nu with small coefficients (Gaussian).
    pub fn sample_rand_vec(&self, rng: &mut impl RngCore) -> Vec<PolyRns> {
        let mut gs = GaussianSampler::new(rng);
        (0..self.params.nu)
            .map(|_| sample_small_poly(self.params.ring_degree, self.params.moduli.clone(), self.params.sigma, &mut gs))
            .collect()
    }
}

impl ExpSerde for AjtaiCommitment {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.value.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let value = Vec::<PolyRns>::deserialize_from(&mut reader)?;
        Ok(Self { value })
    }
}

impl ExpSerde for AjtaiCrs {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.params.serialize_into(&mut writer)?;
        self.a0.serialize_into(&mut writer)?;
        self.a1.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let params = LatticeParams::deserialize_from(&mut reader)?;
        let a0 = Vec::<Vec<PolyRns>>::deserialize_from(&mut reader)?;
        let a1 = Vec::<Vec<PolyRns>>::deserialize_from(&mut reader)?;
        Ok(Self { params, a0, a1 })
    }
}

fn sample_uniform_matrix(
    rows: usize,
    cols: usize,
    n: usize,
    moduli: &RnsModuli,
    rng: &mut impl RngCore,
) -> Vec<Vec<PolyRns>> {
    let mut out = Vec::with_capacity(rows);
    for _ in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for _ in 0..cols {
            let mut p = PolyRns::zero(n, moduli.clone());
            for (k, layer) in p.coeffs.iter_mut().enumerate() {
                let q = moduli.modulus(k);
                for c in layer.iter_mut() {
                    *c = rng.next_u64() % q;
                }
            }
            row.push(p);
        }
        out.push(row);
    }
    out
}

#[cfg(test)]
mod ajtai_commit_parallel_tests {
    use super::*;
    use crate::params::LatticeParams;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn commit_parallel_matches_sequential_large_mu() {
        let _g = ENV_LOCK.lock().unwrap();

        let mut params = LatticeParams::default_small();
        params.mu = 8;

        let mut rng = StdRng::seed_from_u64(42);
        let crs = AjtaiCrs::setup(params, &mut rng);
        let m: Vec<PolyRns> = (0..crs.params.ell)
            .map(|_| PolyRns::zero(crs.params.ring_degree, crs.params.moduli.clone()))
            .collect();
        let r = crs.sample_rand_vec(&mut StdRng::seed_from_u64(7));

        std::env::set_var(ENV_AJTAI_COMMIT_PARALLEL, "0");
        let seq = crs.commit(&m, &r);
        std::env::remove_var(ENV_AJTAI_COMMIT_PARALLEL);
        let par = crs.commit(&m, &r);

        assert_eq!(seq, par);
    }
}

fn sample_small_poly<'a, R: RngCore>(
    n: usize,
    moduli: RnsModuli,
    sigma: f64,
    gs: &mut GaussianSampler<'a, R>,
) -> PolyRns {
    let mut p = PolyRns::zero(n, moduli.clone());
    for i in 0..n {
        let s = gs.sample_i64(sigma);
        for (k, layer) in p.coeffs.iter_mut().enumerate() {
            let q = moduli.modulus(k) as i64;
            let mut v = s % q;
            if v < 0 {
                v += q;
            }
            layer[i] = v as u64;
        }
    }
    p
}

