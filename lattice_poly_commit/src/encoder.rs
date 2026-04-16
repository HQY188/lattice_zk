//! 消息编码 `Ecd`：把基域 `F` 上的一段系数塞进单个 `PolyRns`。
//!
//! **原型约定**：系数按索引 `0..coeffs.len()` 直接写入环多项式同一下标（再对每个 RNS 腿取模），
//! 这样与 `multilinear` 里用 `encode_block(&[scalar])` 做标量环乘一致。解码目前只读第一条 RNS 腿。

use arith::Field;

use crate::{params::LatticeParams, ring::PolyRns};

/// 将系数块编码为 \(R_q\) 中的一个多项式（原型：直接嵌入）。
///
/// Prototype encoding: direct coefficient embedding (no gadget decomposition / slots).
pub fn encode_block<F: Field>(coeffs: &[F], params: &LatticeParams) -> PolyRns {
    assert!(coeffs.len() <= params.n);
    let mut p = PolyRns::zero(params.ring_degree, params.moduli.clone());
    for (i, &c) in coeffs.iter().enumerate() {
        // Embed the field element into `R_q` as a small integer mod each modulus.
        //
        // - Base fields with `FIELD_SIZE <= 64` (e.g. M31, Goldilocks): use the
        //   canonical low limb from `to_u256` (Goldilocks needs the full u64 limb;
        //   `as_u32_unchecked` would panic once `v > u32::MAX`).
        // - Larger / packed extension fields (e.g. M31Ext3, GoldilocksExt): keep
        //   the historical projection to the first base limb via `as_u32_unchecked()`,
        //   matching scalar linear-combine paths elsewhere.
        let v: u64 = if F::FIELD_SIZE > 64 {
            c.as_u32_unchecked() as u64
        } else {
            field_to_u64(c)
        };
        for (k, layer) in p.coeffs.iter_mut().enumerate() {
            let q = params.moduli.modulus(k);
            layer[i] = v % q;
        }
    }
    p
}

/// 从 `PolyRns` 解出系数（仅第一条模数腿，与原型一致）。
pub fn decode_block(p: &PolyRns) -> Vec<u64> {
    // Decode from the first modulus only (prototype).
    p.coeffs
        .first()
        .map(|layer| layer.clone())
        .unwrap_or_default()
}

/// 将域元素压成 `u64`（`to_u256` 的低 64 位字）。
#[inline]
fn field_to_u64<F: Field>(x: F) -> u64 {
    let u = x.to_u256();
    let (_hi, lo) = u.into_words();
    lo as u64
}

