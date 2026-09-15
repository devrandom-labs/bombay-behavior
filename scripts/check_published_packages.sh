#!/usr/bin/env bash
set -euo pipefail

published_packages=(
  bombay-behavior-macros
  bombay-behavior
  bombay-behavior-actors
)

required_files=(
  Cargo.toml
  LICENSE-APACHE
  LICENSE-MIT
  README.md
)

package_mode=${BOMBAY_PACKAGE_MODE:-archive}

case "$package_mode" in
  archive)
    cargo package \
      --package bombay-behavior-macros \
      --package bombay-behavior \
      --package bombay-behavior-actors \
      --locked \
      --allow-dirty \
      --no-verify
    ;;
  list)
    ;;
  *)
    echo "unknown BOMBAY_PACKAGE_MODE: $package_mode" >&2
    exit 2
    ;;
esac

for package in "${published_packages[@]}"; do
  package_files=$(cargo package \
    --package "$package" \
    --locked \
    --allow-dirty \
    --list)

  for required in "${required_files[@]}"; do
    if ! grep -Fxq "$required" <<<"$package_files"; then
      echo "$package does not package $required" >&2
      exit 1
    fi
  done
done
