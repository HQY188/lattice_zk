//! **占位**单变量多项式承诺：承诺就是系数向量，便于在测试/接口层模拟“线性组合可交换”。
//!
//! 真实格实现见 **`univariate_real`**（Ajtai）。本模块保留 `random_poly` 等工具函数供 `multilinear` 采样 `a(X)`。
//! 接口命名对齐 `mle_pc.md`：`setup / commit / open / eval / verify`。

use arith::Field;
use rand::RngCore;
use serdes::{ExpSerde, SerdeResult};

/// Univariate commitment key (degree bound N).
#[derive(Clone, Debug, Default)]
pub struct UniCk<F: Field> {
    _phantom: std::marker::PhantomData<F>,
    pub n: usize,
}

impl<F: Field> ExpSerde for UniCk<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.n.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let n = usize::deserialize_from(&mut reader)?;
        Ok(Self {
            _phantom: std::marker::PhantomData,
            n,
        })
    }
}

impl<F: Field> UniCk<F> {
    pub fn new(n: usize) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            n,
        }
    }
}

/// Commitment = coefficient vector (for stub: supports linear combination in Verify).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniCommitment<F: Field> {
    pub coeffs: Vec<F>,
}

impl<F: Field> UniCommitment<F> {
    pub fn from_coeffs(coeffs: Vec<F>) -> Self {
        Self { coeffs }
    }

    /// Linear combination: self + e * other (coefficient-wise).
    pub fn add_scalar_mul(&self, e: &F, other: &Self) -> Self {
        assert_eq!(self.coeffs.len(), other.coeffs.len());
        let coeffs = self
            .coeffs
            .iter()
            .zip(other.coeffs.iter())
            .map(|(a, b)| *a + *e * *b)
            .collect();
        Self { coeffs }
    }
}

impl<F: Field + ExpSerde> ExpSerde for UniCommitment<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.coeffs.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let coeffs = Vec::deserialize_from(&mut reader)?;
        Ok(Self { coeffs })
    }
}

/// Opening info δ = (coefficients, eta_stub). Stub does not use eta.
#[derive(Clone, Debug, Default)]
pub struct UniOpening<F: Field> {
    pub coeffs: Vec<F>,
}

impl<F: Field + ExpSerde> ExpSerde for UniOpening<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.coeffs.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let coeffs = Vec::deserialize_from(&mut reader)?;
        Ok(Self { coeffs })
    }
}

/// Evaluate univariate polynomial at x (coefficients h_0, h_1, ... => sum h_i x^i).
#[inline]
#[allow(dead_code)]
pub fn univariate_eval<F: Field>(coeffs: &[F], x: &F) -> F {
    if coeffs.is_empty() {
        return F::ZERO;
    }
    let mut y = coeffs[coeffs.len() - 1];
    for i in (0..coeffs.len().saturating_sub(1)).rev() {
        y = y * *x + coeffs[i];
    }
    y
}

/// PC.Setup(1^λ, N) -> ck
#[allow(dead_code)]
pub fn setup<F: Field>(_lambda: usize, n: usize) -> UniCk<F> {
    UniCk::new(n)
}

/// PC.Com(ck, h(X)) -> (c, δ). Stub: c = coeffs, δ = (coeffs,).
#[allow(dead_code)]
pub fn commit<F: Field>(ck: &UniCk<F>, coeffs: &[F]) -> (UniCommitment<F>, UniOpening<F>) {
    assert!(coeffs.len() <= ck.n);
    let mut c = coeffs.to_vec();
    c.resize(ck.n, F::ZERO);
    let delta = UniOpening {
        coeffs: c.clone(),
    };
    (UniCommitment { coeffs: c }, delta)
}

/// PC.Open(ck, c, h, δ): check c equals polynomial coefficients.
#[allow(dead_code)]
pub fn open<F: Field>(
    _ck: &UniCk<F>,
    c: &UniCommitment<F>,
    coeffs: &[F],
    delta: &UniOpening<F>,
) -> bool {
    let mut h = coeffs.to_vec();
    h.resize(c.coeffs.len(), F::ZERO);
    delta.coeffs == h && c.coeffs == h
}

/// PC.Eval(x, δ) -> (y, ρ). Stub: y = h(x), ρ = (coeffs) so Verify can check.
#[allow(dead_code)]
pub fn eval<F: Field>(x: &F, delta: &UniOpening<F>) -> (F, UniOpening<F>) {
    let y = univariate_eval(&delta.coeffs, x);
    (y, delta.clone())
}

/// PC.Verify(ck, c, x, y, ρ): y = poly(ρ)(x) and c == ρ.
#[allow(dead_code)]
pub fn verify<F: Field>(
    _ck: &UniCk<F>,
    c: &UniCommitment<F>,
    x: &F,
    y: F,
    rho: &UniOpening<F>,
) -> bool {
    univariate_eval(&rho.coeffs, x) == y && c.coeffs == rho.coeffs
}

/// Sample random polynomial of degree < n (for prover's a(X) in MLE Eval).
pub fn random_poly<F: Field>(n: usize, rng: &mut impl RngCore) -> Vec<F> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(F::random_unsafe(&mut *rng));
    }
    out
}
