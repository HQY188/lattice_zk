use rand::RngCore;
use serdes::{ExpSerde, SerdeResult};

use crate::{
    params::LatticeParams,
    ring::{PolyRns, RnsModuli},
    sampler::GaussianSampler,
};

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
    pub fn commit(&self, m: &[PolyRns], r: &[PolyRns]) -> AjtaiCommitment {
        assert_eq!(m.len(), self.params.ell);
        assert_eq!(r.len(), self.params.nu);
        let mut out = AjtaiCommitment::zero(self.params.mu, self.params.ring_degree, self.params.moduli.clone());
        let ntt = self.params.ntt_plan();

        for i in 0..self.params.mu {
            // sum_j A0[i][j] * m[j] + sum_j A1[i][j] * r[j]
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

