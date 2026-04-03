$env:VIRGO_M31_CIRCUIT = "D:\path\to\virgo_m31_circuit.txt"
$env:VIRGO_M31_META    = "D:\path\to\virgo_m31_meta.txt"
$env:VIRGO_M31_LOG     = "D:\CS\ZK\work\Expander\results\virgo_m31.log"
# babybear 同理设置 VIRGO_BABYBEAR_*
powershell -ExecutionPolicy Bypass -File scripts\run_virgo.ps1

# 仅看说明（未配环境变量且未 fallback）
powershell -ExecutionPolicy Bypass -File scripts\run_virgo.ps1

# 占位跑 Virgo 自带 SHA256（需已生成 tests/SHA256 下文件）
powershell -ExecutionPolicy Bypass -File scripts\run_virgo.ps1 -UseSha256Fallback

cargo run -p expander_to_virgo --release -- `
  --input ".\data\circuit_m31.txt" --field m31 --out-dir ".\Virgo\data" --prefix keccak_m31 --witness ".\data\witness_m31.txt"

cargo run -p expander_to_virgo --release -- `
  --input ".\data\circuit_babybear.txt" --field babybear --out-dir ".\Virgo\data" --prefix keccak_babybear --witness ".\data\witness_babybear.txt"