use arith::Field;

use crate::{params::LatticeParams, ring::PolyRns};

/// Encode a block of coefficients (in base field F) into a single ring element in R_q.
///
/// Prototype encoding: direct coefficient embedding (no gadget decomposition / slots).
pub fn encode_block<F: Field>(coeffs: &[F], params: &LatticeParams) -> PolyRns {
    assert!(coeffs.len() <= params.n);
    let mut p = PolyRns::zero(params.ring_degree, params.moduli.clone());
    for (i, &c) in coeffs.iter().enumerate() {
        // For extension-field challenges (e.g. M31Ext3), committing needs a stable
        // scalar representation that is consistent across commit/open and across
        // scalar linear combinations on the ring side.
        //
        // We intentionally project the field element to a single base-limb via
        // `as_u32_unchecked()`, so that Ecd is compatible with the ring's
        // constant-scalar multiplication used in multilinear eval/verify.
        let v = c.as_u32_unchecked() as u64;
        for (k, layer) in p.coeffs.iter_mut().enumerate() {
            let q = params.moduli.modulus(k);
            layer[i] = v % q;
        }
    }
    p
}

pub fn decode_block(p: &PolyRns) -> Vec<u64> {
    // Decode from the first modulus only (prototype).
    p.coeffs
        .first()
        .map(|layer| layer.clone())
        .unwrap_or_default()
}

#[inline]
fn field_to_u64<F: Field>(x: F) -> u64 {
    let u = x.to_u256();
    let (_hi, lo) = u.into_words();
    lo as u64
}

