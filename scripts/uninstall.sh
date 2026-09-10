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
rm -f /etc/workflowd/workflowd.env
# Preserve the root-owned master key beside the state it unlocks.
rmdir /etc/workflowd 2>/dev/null || true
rm -rf /usr/share/doc/workflowd
systemctl daemon-reload
systemctl reset-failed workflowd.service 2>/dev/null || true
printf '%s\n' "Canopy Workbench was removed; /var/lib/workflow-rust and /etc/workflowd/master.key were preserved."
