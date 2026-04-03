#!/usr/bin/env bash
# Virgo zk_proof：与 scripts/run_virgo.ps1 对齐。
# 优先 VIRGO_M31_* / VIRGO_BABYBEAR_*；否则若存在 data/circuit_{m31,babybear}.txt 且已构建 expander_to_virgo，则依次测 m31 与 babybear；
# 可选 --sha256-fallback 使用 Virgo/tests/SHA256 预生成电路（非 Expander data）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VIRGO="${ROOT}/Virgo"
SHA="${VIRGO}/tests/SHA256"
RESULTS="${ROOT}/results"
mkdir -p "${RESULTS}"

find_zk_proof() {
  for p in \
    "${VIRGO}/build-linux/zk_proof" \
    "${VIRGO}/build-mingw/zk_proof" \
    "${VIRGO}/zk_proof" \
    "${VIRGO}/build/zk_proof" \
    "${SHA}/zk_proof"; do
    if [[ -x "$p" ]]; then echo "$p"; return 0; fi
  done
  return 1
}

find_expander_to_virgo() {
  for p in "${ROOT}/target/release/expander_to_virgo" "${ROOT}/target/debug/expander_to_virgo"; do
    if [[ -x "$p" ]]; then echo "$p"; return 0; fi
  done
  return 1
}

ZK="$(find_zk_proof || true)"
if [[ -z "${ZK}" ]]; then
  echo "未找到可执行的 zk_proof，请先在 Virgo 目录用 CMake 编译。" >&2
  exit 1
fi

echo "[virgo] zk_proof: ${ZK}"

run_case() {
  local label="$1" circuit="$2" meta="$3" log="$4"
  echo ""
  echo "[virgo] ${label}"
  echo "  circuit=${circuit}"
  echo "  meta=${meta}"
  echo "  log=${log}"
  "${ZK}" "${circuit}" "${meta}" "${log}"
}

DUMMY_META="dummy_meta.txt"

run_expander_field() {
  local field="$1" circuit="$2" witness="$3" log="$4"
  export EXPANDER_FIELD="${field}"
  if [[ -f "${witness}" ]]; then
    export EXPANDER_WITNESS="${witness}"
    echo "[virgo] EXPANDER_FIELD=${field}, EXPANDER_WITNESS=${witness}"
  else
    unset EXPANDER_WITNESS || true
    echo "[virgo] EXPANDER_FIELD=${field} (no witness file)"
  fi
  local name
  name="$(basename "${circuit}")"
  run_case "data/${name} (Expander binary -> Virgo)" "${circuit}" "${DUMMY_META}" "${log}"
}

USED=0
if [[ -n "${VIRGO_M31_CIRCUIT:-}" && -n "${VIRGO_M31_META:-}" && -n "${VIRGO_M31_LOG:-}" ]]; then
  export EXPANDER_FIELD=m31
  if [[ -f "${ROOT}/data/witness_m31.txt" ]]; then export EXPANDER_WITNESS="${ROOT}/data/witness_m31.txt"; fi
  run_case "m31 (env)" "${VIRGO_M31_CIRCUIT}" "${VIRGO_M31_META}" "${VIRGO_M31_LOG}"
  USED=1
fi
if [[ -n "${VIRGO_BABYBEAR_CIRCUIT:-}" && -n "${VIRGO_BABYBEAR_META:-}" && -n "${VIRGO_BABYBEAR_LOG:-}" ]]; then
  export EXPANDER_FIELD=babybear
  if [[ -f "${ROOT}/data/witness_babybear.txt" ]]; then export EXPANDER_WITNESS="${ROOT}/data/witness_babybear.txt"; fi
  run_case "babybear (env)" "${VIRGO_BABYBEAR_CIRCUIT}" "${VIRGO_BABYBEAR_META}" "${VIRGO_BABYBEAR_LOG}"
  USED=1
fi

if [[ "${USED}" -eq 1 ]]; then
  echo ""
  echo "[virgo] OK (environment variables)."
  exit 0
fi

if [[ "${1:-}" == "--sha256-fallback" ]]; then
  M1C="${SHA}/SHA256_64_merkle_1_circuit.txt"
  M1M="${SHA}/SHA256_64_merkle_1_meta.txt"
  M2C="${SHA}/SHA256_64_merkle_2_circuit.txt"
  M2M="${SHA}/SHA256_64_merkle_2_meta.txt"
  if [[ -f "${M1C}" && -f "${M1M}" && -f "${M2C}" && -f "${M2M}" ]]; then
    echo "警告: 使用 SHA256 预生成电路（非 Expander data/）作为占位。" >&2
    run_case "fallback merkle_1" "${M1C}" "${M1M}" "${RESULTS}/virgo_fallback_merkle1.log"
    run_case "fallback merkle_2" "${M2C}" "${M2M}" "${RESULTS}/virgo_fallback_merkle2.log"
    echo ""
    echo "[virgo] OK (SHA256 fallback)."
    exit 0
  fi
  echo "缺少 ${SHA}/SHA256_64_merkle_{1,2}_*.txt，请先在该目录执行 Virgo 官方 build。" >&2
  exit 1
fi

M31_C="${ROOT}/data/circuit_m31.txt"
BB_C="${ROOT}/data/circuit_babybear.txt"
M31_W="${ROOT}/data/witness_m31.txt"
BB_W="${ROOT}/data/witness_babybear.txt"
M31_LOG="${RESULTS}/virgo_data_m31.log"
BB_LOG="${RESULTS}/virgo_data_babybear.log"

CONV="$(find_expander_to_virgo || true)"
if [[ -f "${M31_C}" || -f "${BB_C}" ]]; then
  if [[ -z "${CONV}" ]]; then
    echo "已找到 data/circuit_*.txt，但未找到 expander_to_virgo。请执行: cargo build -p expander_to_virgo --release" >&2
    exit 1
  fi
  export EXPANDER_TO_VIRGO="${CONV}"
  echo "[virgo] EXPANDER_TO_VIRGO=${CONV}"
  if [[ -f "${M31_C}" ]]; then
    run_expander_field "m31" "${M31_C}" "${M31_W}" "${M31_LOG}"
  fi
  if [[ -f "${BB_C}" ]]; then
    run_expander_field "babybear" "${BB_C}" "${BB_W}" "${BB_LOG}"
  fi
  echo ""
  echo "[virgo] OK (data/: m31 and/or babybear via expander_to_virgo)."
  exit 0
fi

cat >&2 <<EOF
未设置 VIRGO_M31_* / VIRGO_BABYBEAR_*，且无 data/circuit_m31.txt / data/circuit_babybear.txt；
也未使用 --sha256-fallback。

可选：
  cargo build -p expander_to_virgo --release
  并放置 data/circuit_m31.txt 与/或 data/circuit_babybear.txt 后重试本脚本；
或：
  $0 --sha256-fallback   （需先构建 Virgo/tests/SHA256 测试文件）
EOF
exit 1
