#!/usr/bin/env bash
# Two-point Virgo zk_proof e2e: SHA256 merkle_1/2 if present, else matmul gen 2 and 4.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VIRGO="${ROOT}/Virgo"
SHA="${VIRGO}/tests/SHA256"
MM="${VIRGO}/tests/matmul"
RESULTS="${ROOT}/results"
mkdir -p "${RESULTS}"

find_zk() {
  for p in "${VIRGO}/build-linux/zk_proof" "${VIRGO}/build-mingw/zk_proof" "${SHA}/zk_proof" "${VIRGO}/zk_proof"; do
    if [[ -x "$p" ]]; then echo "$p"; return 0; fi
  done
  return 1
}

ZK="$(find_zk || true)"
if [[ -z "${ZK}" ]]; then
  echo "zk_proof not found. Build Virgo (see scripts/build_virgo.sh)." >&2
  exit 1
fi

run_one() {
  local name="$1" c="$2" m="$3" log="$4"
  echo ""
  echo "[virgo-e2e] ${name}"
  echo "  circuit=${c}"
  echo "  meta=${m}"
  echo "  log=${log}"
  "${ZK}" "${c}" "${m}" "${log}"
}

M1C="${SHA}/SHA256_64_merkle_1_circuit.txt"
M1M="${SHA}/SHA256_64_merkle_1_meta.txt"
M2C="${SHA}/SHA256_64_merkle_2_circuit.txt"
M2M="${SHA}/SHA256_64_merkle_2_meta.txt"

if [[ -f "${M1C}" && -f "${M1M}" && -f "${M2C}" && -f "${M2M}" ]]; then
  echo "[virgo-e2e] Mode: SHA256 merkle_1 + merkle_2"
  run_one "merkle_1" "${M1C}" "${M1M}" "${RESULTS}/virgo_e2e_sha256_merkle1.log"
  run_one "merkle_2" "${M2C}" "${M2M}" "${RESULTS}/virgo_e2e_sha256_merkle2.log"
  echo ""
  echo "[virgo-e2e] OK (2/2 SHA256)."
  exit 0
fi

echo "[virgo-e2e] SHA256 inputs missing; matmul gen n=2 and n=4"
command -v g++ >/dev/null 2>&1 || { echo "g++ required" >&2; exit 1; }

pushd "${MM}" >/dev/null
g++ gen.cpp -o gen -O3
./gen 2 mat_2_circuit.txt mat_2_meta.txt
./gen 4 mat_4_circuit.txt mat_4_meta.txt
popd >/dev/null

run_one "matmul n=2" "${MM}/mat_2_circuit.txt" "${MM}/mat_2_meta.txt" "${RESULTS}/virgo_e2e_matmul_2.log"
run_one "matmul n=4" "${MM}/mat_4_circuit.txt" "${MM}/mat_4_meta.txt" "${RESULTS}/virgo_e2e_matmul_4.log"

echo ""
echo "[virgo-e2e] OK (2/2 matmul)."
exit 0
