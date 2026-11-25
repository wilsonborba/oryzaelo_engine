#!/usr/bin/env bash
# Runs 5 independent benchmark processes (one round each) and records the
# environment before and after every round. Same script on both hosts.
# Usage: run_rounds.sh <binary> <artifacts_dir> <out_dir> <host_label>
set -euo pipefail
BIN="$1"; ART="$2"; OUT="$3"; HOST="$4"
mkdir -p "$OUT"
ENV="$OUT/environment.txt"

snap() {
  echo "--- $1 $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "loadavg: $(cat /proc/loadavg)"
  echo "governor: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo unavailable)"
  echo "cur_freq_khz: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq 2>/dev/null || echo unavailable)"
  if command -v vcgencmd >/dev/null 2>&1; then
    echo "temp: $(vcgencmd measure_temp)"; echo "throttled: $(vcgencmd get_throttled)"
  else
    echo "temp: unavailable (no vcgencmd)"; echo "throttled: unavailable (no vcgencmd)"
  fi
  echo "oryzaelo_engine service: $(systemctl is-active oryzaelo_engine 2>/dev/null || echo not-installed)"
  echo "top cpu processes:"; ps -eo pid,comm,%cpu,rss --sort=-%cpu | head -6
}

{
  echo "host_label: $HOST"; echo "hostname: $(hostname)"; echo "kernel: $(uname -srm)"
  grep PRETTY_NAME /etc/os-release || true
  [ -r /proc/device-tree/model ] && echo "model: $(tr -d '\0' </proc/device-tree/model)"
  lscpu | grep -E "Model name|^CPU\(s\)|max MHz|min MHz" || true
  free -m | head -2
  echo "binary sha256: $(sha256sum "$BIN" | cut -c1-64)"
  echo "artifact hashes:"; (cd "$ART" && sha256sum rice_stage_six.onnx model_metadata.json benchmark_input.json pi_benchmark/reference_probabilities.json)
} > "$ENV"

for r in 1 2 3 4 5; do
  snap "before round $r" >> "$ENV"
  "$BIN" --model "$ART/rice_stage_six.onnx" --metadata "$ART/model_metadata.json" \
         --input "$ART/benchmark_input.json" --reference "$ART/pi_benchmark/reference_probabilities.json" \
         --out-dir "$OUT" --round "$r" --host "$HOST" --warmup 50 --iterations 1000
  snap "after round $r" >> "$ENV"
done
echo "done: $OUT"
