#!/usr/bin/env bash
# Linux / WSL：与 Virgo 官方 README、setup.sh 一致，使用 GCC/Clang 编译 zk_proof。
#
# 【逐步】cd 到仓库根 -> Virgo 子模块 init -> 检查 include/lib ->
# cmake -S Virgo -B Virgo/build-linux ->仅构建目标 zk_proof。
#
# 用法：在 Expander 仓库根目录执行  ./scripts/build_virgo.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VIRGO="${ROOT}/Virgo"
BUILD="${VIRGO}/build-linux"

echo "[build_virgo] VIRGO=${VIRGO}"

if [[ -d "${VIRGO}/.git" ]]; then
  git -C "${VIRGO}" submodule update --init --recursive
fi

if [[ ! -d "${VIRGO}/include/flo-shani-aesni" ]]; then
  echo "缺少 ${VIRGO}/include/flo-shani-aesni，请执行: git -C Virgo submodule update --init --recursive" >&2
  exit 1
fi

if [[ ! -f "${VIRGO}/lib/libflo-shani.a" ]]; then
  echo "缺少 ${VIRGO}/lib/libflo-shani.a" >&2
  exit 1
fi
if [[ ! -f "${VIRGO}/include/XKCP/Standalone/CompactFIPS202/C/Keccak-more-compact.c" ]]; then
  echo "缺少 XKCP Compact submodule（SHA3 嵌入编译，无需 libXKCP.a）" >&2
  exit 1
fi

cmake -S "${VIRGO}" -B "${BUILD}" -DCMAKE_BUILD_TYPE=Release
cmake --build "${BUILD}" --target zk_proof -j "$(nproc 2>/dev/null || echo 4)"

OUT="${BUILD}/zk_proof"
if [[ -x "${OUT}" ]]; then
  echo "[build_virgo] OK: ${OUT}"
  echo "运行: ${ROOT}/scripts/run_virgo.sh"
else
  echo "[build_virgo] 未找到可执行文件: ${OUT}" >&2
  exit 1
fi
