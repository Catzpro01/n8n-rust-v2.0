#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "uninstall.sh must run as root" >&2
  exit 1
fi

systemctl disable --now workflowd.service 2>/dev/null || true
rm -f /etc/systemd/system/workflowd.service
rm -f /usr/bin/workflowd
rm -rf /etc/workflowd
rm -rf /usr/share/doc/workflowd
systemctl daemon-reload
systemctl reset-failed workflowd.service 2>/dev/null || true
printf '%s\n' "Canopy Workbench was removed; /var/lib/workflow-rust was preserved."
