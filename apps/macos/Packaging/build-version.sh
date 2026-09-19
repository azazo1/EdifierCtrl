#!/usr/bin/env bash
set -euo pipefail

# 仅发布打包调用. 无版本 tag 时按项目约定返回 dev-build.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root"
exact="$(git tag --points-at HEAD --sort=-version:refname | /usr/bin/awk '/^v?[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$/ { print; exit }')"
latest="$exact"
if [[ -z "$latest" ]]; then
    latest="$(git describe --tags --abbrev=0 --match 'v[0-9]*.[0-9]*.[0-9]*' --match '[0-9]*.[0-9]*.[0-9]*' HEAD 2>/dev/null || true)"
fi
if [[ -z "$latest" ]]; then
    printf 'dev-build\n'
    exit 0
fi
commit="$(git rev-parse --short=7 HEAD)"
dirty=false
if ! git diff-index --quiet HEAD --; then dirty=true; fi
if [[ -n "$(git ls-files --others --exclude-standard)" ]]; then dirty=true; fi
if [[ "$dirty" == true ]]; then
    printf '%s^%s\n' "$latest" "$commit"
elif [[ -n "$exact" ]]; then
    printf '%s\n' "$exact"
else
    printf '%s-%s\n' "$latest" "$commit"
fi
