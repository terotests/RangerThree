# Sourced by the scripts: puts Ranger (the compiler and the `ranger` prelude
# crate the Cargo.toml files name) at .deps/Ranger, at the commit deps.json
# pins. RANGER_DIR names a checkout of your own instead (then .deps/Ranger
# becomes a link to it). ensure_componentengine does the same for CErXes.
_DEPS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

_deps_pin() { # <dep> <field>
  node -e 'const m=require(process.argv[1]);process.stdout.write((m[process.argv[2]]||{})[process.argv[3]]||"")' \
    "$_DEPS_ROOT/deps.json" "$1" "$2"
}

# <name> <dir> <env override>: <dir> at the pin, or a link to the override
_deps_sync() {
  local name="$1" dir="$2" own="$3" url rev head=""
  if [[ -n "$own" ]]; then
    [[ -L "$dir" || ! -e "$dir" ]] || { echo "$dir exists and is not a link; remove it to use $own" >&2; return 1; }
    mkdir -p "$(dirname "$dir")"
    ln -sfn "$(cd "$own" && pwd)" "$dir"
    return 0
  fi
  [[ -L "$dir" ]] && return 0
  url="$(_deps_pin "$name" git)"
  rev="$(_deps_pin "$name" rev)"
  if [[ ! -d "$dir/.git" ]]; then
    mkdir -p "$dir"
    git -C "$dir" init -q
    git -C "$dir" remote add origin "$url"
  else
    head="$(git -C "$dir" rev-parse -q --verify HEAD 2>/dev/null || true)"
  fi
  [[ "$head" == "$rev" ]] && return 0
  echo "==> $name -> ${rev:0:7}   ($dir)" >&2
  git -C "$dir" fetch -q --depth 1 origin "$rev"
  git -C "$dir" checkout -q -f --detach FETCH_HEAD
}

ensure_ranger() {
  _deps_sync ranger "$_DEPS_ROOT/.deps/Ranger" "${RANGER_DIR:-}" || return 1
  RANGER="$_DEPS_ROOT/.deps/Ranger"
  if [[ ! -f "$RANGER/dist/rgrc.js" ]]; then
    echo "no $RANGER/dist/rgrc.js" >&2
    return 1
  fi
}

ensure_componentengine() {
  _deps_sync componentengine "$_DEPS_ROOT/.deps/componentengine" "${COMPONENTENGINE_DIR:-}" || return 1
  COMPONENTENGINE="$_DEPS_ROOT/.deps/componentengine"
}

# The CErXes command line (runs a .js file), built from componentengine.
ensure_cerxes_cli() {
  ensure_componentengine || return 1
  CERXES_BIN="${CERXES_BIN:-$_DEPS_ROOT/tmp/cargo-cerxes/release/cerxes}"
  if [[ ! -x "$CERXES_BIN" ]]; then
    (cd "$COMPONENTENGINE/cerxes" && CARGO_TARGET_DIR="$_DEPS_ROOT/tmp/cargo-cerxes" cargo build -q --release --bin cerxes) || return 1
  fi
}
