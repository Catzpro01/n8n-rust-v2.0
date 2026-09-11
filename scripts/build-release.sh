#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo"
export PATH="$HOME/.cargo/bin:$HOME/.local/node-v22.19.0-linux-x64/bin:$PATH"
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}
export WORKFLOWD_BUILD_COMMIT=${WORKFLOWD_BUILD_COMMIT:-$(git rev-parse HEAD)$(git diff --quiet && git diff --cached --quiet || printf '%s' '-dirty')}
bundle=${1:-out/tracer-bundle}
rm -rf "$bundle"
mkdir -p "$bundle"

(cd editor && npm ci && npm run typecheck && npm run build && npm audit --audit-level=high)
cargo +1.85.1 build --workspace --release --frozen

install -D -m 0755 target/release/workflowd "$bundle/usr/bin/workflowd"
strip --strip-all "$bundle/usr/bin/workflowd"
install -D -m 0644 packaging/systemd/workflowd.service \
  "$bundle/usr/lib/systemd/system/workflowd.service"
install -D -m 0644 packaging/workflowd.env "$bundle/etc/workflowd/workflowd.env"
install -D -m 0644 LICENSE "$bundle/usr/share/doc/workflowd/LICENSE"
cp -a LICENSES "$bundle/usr/share/doc/workflowd/"
install -D -m 0644 contracts/manual-trigger.v1alpha1.json "$bundle/usr/share/workflowd/contracts/manual-trigger.v1alpha1.json"
install -D -m 0644 contracts/generate-items.v1alpha1.json "$bundle/usr/share/workflowd/contracts/generate-items.v1alpha1.json"
mkdir -p "$bundle/usr/share/workflowd/sdk"
cp -a sdk/node-contract "$bundle/usr/share/workflowd/sdk/"

python3 tools/release_metadata.py "$bundle"
checksum_file=$(mktemp)
trap 'rm -f "$checksum_file"' EXIT
(
  cd "$bundle"
  find . -type f ! -name checksums.sha256 -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    | sed 's#  \./#  #'
) > "$checksum_file"
mv "$checksum_file" "$bundle/checksums.sha256"
trap - EXIT

tarball="${bundle%/}.tar.gz"
rm -f "$tarball"
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 \
  --numeric-owner -czf "$tarball" -C "$(dirname "$bundle")" "$(basename "$bundle")"
printf 'bundle=%s\ntarball=%s\nbinary_bytes=%s\n' \
  "$bundle" "$tarball" "$(stat -c %s "$bundle/usr/bin/workflowd")"
