#!/usr/bin/env bash
# Runs the latest debug build of blackglass on the example vault.
#
#   ./run.sh                         # the example vault
#   ./run.sh Plugins/Dataview.md     # a note in it (a path inside example_vault/)
#   ./run.sh --no-mouse              # options are passed on
#
# Builds first (cargo only rebuilds what changed), so it's always current.
# Uses a config folder of its own (target/run-config; BLACKGLASS_RUN_CONFIG
# to choose another), so trying things never changes your settings or the
# vault blackglass opens last.
set -euo pipefail
cd "$(dirname "$0")"
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

cargo build --quiet

# A note name (not an option) is looked for in the example vault.
args=()
target="example_vault"
for arg in "$@"; do
    if [[ "$arg" != -* ]]; then
        target="example_vault/$arg"
    else
        args+=("$arg")
    fi
done
if [[ ! -e "$target" ]]; then
    echo "run.sh: $target doesn't exist" >&2
    exit 1
fi

export XDG_CONFIG_HOME="${BLACKGLASS_RUN_CONFIG:-$PWD/target/run-config}"
exec target/debug/blackglass "${args[@]}" "$target"
