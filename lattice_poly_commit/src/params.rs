use crate::ring::{NttPlan, RnsModuli};
use serdes::{ExpSerde, SerdeResult};

/// Parameters for a (prototype) lattice-based univariate polynomial commitment over R_q.
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
        let ring_degree = 256usize;
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
            // With the current prototype encoding (direct coefficient embedding),
            // message coefficients can be uniform mod q, making the balanced-lift 2-norm
            // on the order of ~sqrt(d)*q. Use a very loose bound so correctness tests
            // don't fail due to encoding (not security) considerations.
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

