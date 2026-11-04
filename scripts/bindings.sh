#!/bin/sh
# Build notav-ffi for this machine, generate its Kotlin bindings into notav/ffi/target/bindings
# and run the smoke test against them (with the Android project's Gradle wrapper, on a JDK 21,
# and msgfmt), as Todav's does.
#
#     scripts/bindings.sh            # generate and test
#     scripts/bindings.sh --no-test  # generate only
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
ffi="$root/notav/ffi"
cd "$ffi"

cargo build --locked
cargo run --locked -q --features bindgen --bin uniffi-bindgen -- \
  generate --library target/debug/libnotav_ffi.so --language kotlin \
  --out-dir target/bindings/kotlin --no-format

[ "${1:-}" = --no-test ] && exit 0

# A catalog for the smoke test to read.
msgfmt -o target/bindings/pl.mo "$root/po/pl.po"

export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk}"
"$root/notav/android/gradlew" -q -p tests/kotlin run --console=plain
