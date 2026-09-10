#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo"
bundle=${1:-out/tracer-bundle}
port=${WORKFLOWD_SMOKE_PORT:-18787}
origin="http://127.0.0.1:$port"

cleanup() {
  sudo ./scripts/uninstall.sh >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup
sudo rm -rf /var/lib/workflow-rust /run/workflow-rust /etc/workflowd
sudo ./scripts/install.sh --bundle "$bundle" --bind "127.0.0.1:$port"

for _ in {1..100}; do
  curl -fsS "$origin/health/ready" >/dev/null 2>&1 && break
  sleep 0.1
done
curl -fsS "$origin/health/ready" | jq -e \
  '.status == "ready" and .checks.sqlite.journal_mode == "wal" and .checks.sqlite.synchronous == "full"' >/dev/null
curl -fsS "$origin/" | grep -q 'Canopy Workbench'
curl -fsS "$origin/api/v1/release" | jq -e \
  '.product == "Canopy Workbench" and .api_version == "v1"' >/dev/null

setup=$(curl -fsS --request POST "$origin/api/v1/setup" \
  --header "Origin: $origin" --header 'Content-Type: application/json' \
  --data '{"email":"owner@systemd.test","password":"systemd smoke password 2026","recovery_passphrase":"separate systemd recovery phrase 2026"}')
kit_document=$(jq -r '.recovery_kit.document' <<<"$setup")
kit_checksum=$(jq -r '.recovery_kit.checksum' <<<"$setup")
[[ $(printf '%s' "$kit_document" | sha256sum | cut -d' ' -f1) == "$kit_checksum" ]]
curl -fsS "$origin/health/ready" | jq -e \
  '.recovery.state == "recovery-kit-unacknowledged"' >/dev/null

[[ $(systemctl show workflowd.service -p User --value) == workflowd ]]
[[ $(systemctl show workflowd.service -p Group --value) == workflowd ]]
[[ $(systemctl show workflowd.service -p MemoryMax --value) == 524288000 ]]
[[ $(systemctl show workflowd.service -p MemorySwapMax --value) == 0 ]]
[[ $(systemctl show workflowd.service -p TasksMax --value) == 64 ]]
[[ $(systemctl show workflowd.service -p CPUQuotaPerSecUSec --value) == 500ms ]]
[[ $(sudo stat -c '%U:%G:%a:%s' /etc/workflowd/master.key) == root:root:600:32 ]]
systemd-analyze verify workflowd.service
exposure=$(systemd-analyze security --no-pager workflowd.service \
  | awk '/Overall exposure level/ {print $(NF-2)}')
awk -v score="$exposure" 'BEGIN {exit !(score <= 3.0)}'

pid=$(systemctl show workflowd.service -p MainPID --value)
[[ $pid =~ ^[1-9][0-9]*$ ]]
[[ $(awk '/^Uid:/ {print $2}' "/proc/$pid/status") == $(id -u workflowd) ]]
[[ $(pgrep -u workflowd -x workflowd | wc -l) -eq 1 ]]
resources=$(curl -fsS "$origin/api/v1/resources")
jq -e '.cpu.available and .cpu.values.quota_cores == 0.5' <<<"$resources" >/dev/null
jq -e '.memory.available and .memory.values.max_bytes == 524288000 and .memory.values.swap_max_bytes == 0' <<<"$resources" >/dev/null
jq -e '.tasks.available and .tasks.values.max == 64' <<<"$resources" >/dev/null

rss_bytes=$(awk '/VmRSS:/ {print $2 * 1024}' "/proc/$pid/status")
(( rss_bytes < 524288000 ))
cpu_ticks_before=$(awk '{print $14 + $15}' "/proc/$pid/stat")
sleep 2
cpu_ticks_after=$(awk '{print $14 + $15}' "/proc/$pid/stat")
clock_ticks=$(getconf CLK_TCK)
idle_cpu_cores=$(awk -v used="$((cpu_ticks_after - cpu_ticks_before))" \
  -v ticks="$clock_ticks" 'BEGIN {printf "%.4f", used / ticks / 2}')
awk -v cores="$idle_cpu_cores" 'BEGIN {exit !(cores <= 0.5)}'
sudo -u workflowd touch /var/lib/workflow-rust/install-smoke-marker
sudo systemctl restart workflowd.service
for _ in {1..100}; do
  curl -fsS "$origin/health/ready" >/dev/null 2>&1 && break
  sleep 0.1
done
curl -fsS "$origin/health/ready" >/dev/null
sudo test -f /var/lib/workflow-rust/install-smoke-marker

sudo ./scripts/uninstall.sh >/dev/null
[[ ! -e /usr/bin/workflowd ]]
[[ ! -e /etc/systemd/system/workflowd.service ]]
sudo test -f /var/lib/workflow-rust/workflow.sqlite3
sudo test -f /var/lib/workflow-rust/install-smoke-marker
sudo test -f /etc/workflowd/master.key
trap - EXIT
printf 'systemd-smoke=passed state-preserved=/var/lib/workflow-rust/workflow.sqlite3 rss_bytes=%s idle_cpu_cores=%s hardening_exposure=%s\n' \
  "$rss_bytes" "$idle_cpu_cores" "$exposure"
sudo rm -rf /var/lib/workflow-rust /run/workflow-rust /etc/workflowd
