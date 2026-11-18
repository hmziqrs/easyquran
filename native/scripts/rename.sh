#!/usr/bin/env bash
# Rename the app's name and id everywhere they live:
#
#   scripts/rename.sh <new-name> <new-id>      rename this checkout in place
#   scripts/rename.sh --roundtrip-test [NAME] [ID]
#       copy the repo (without target/, .nsh/, .git/) to a sibling directory,
#       rename the copy, run the criterion-1 gate there (fmt, clippy,
#       cargo test, nsh test), report pass/fail, and always delete the copy.
#       Exit 0 only on pass. The copy sits beside this repo so the
#       ../nsh-query and ../nsh-form path dependencies still resolve.
#
# Placeholders renamed: nsh.json (name, id), the Cargo package name and the
# Rust crate ident, the `#[native_module]` specifier prefix, src/main.rs
# (window title, app id), i18n `app.name`, and every other text file that
# mentions them (README, docs, tests, packages). Cargo.lock is left to cargo,
# which rewrites it on the next cargo command.
set -euo pipefail

die() {
    echo "rename.sh: $*" >&2
    exit 1
}

repo_root() {
    cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

# The identity lives in the root nsh.json only; a package's own nsh.json keeps
# its own name.
old_name() {
    sed -n 's/^[[:space:]]*"name"[[:space:]]*:[[:space:]]*"\(.*\)".*/\1/p' "$1/nsh.json" | head -n 1
}

old_id() {
    sed -n 's/^[[:space:]]*"id"[[:space:]]*:[[:space:]]*"\(.*\)".*/\1/p' "$1/nsh.json" | head -n 1
}

# The roundtrip copy's path, at script scope: the EXIT trap fires after
# roundtrip() returned, when its locals no longer exist.
COPY_DIR=""

cleanup() {
    if [ -n "$COPY_DIR" ]; then
        rm -rf "$COPY_DIR"
    fi
}

trap cleanup EXIT

check_name() {
    [[ "$1" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] ||
        die "new name '$1' must be kebab-case: lowercase letters, digits, single hyphens (got: $1)"
}

check_id() {
    [[ "$1" =~ ^[a-z][a-z0-9-]*(\.[a-z0-9-]+)+$ ]] ||
        die "new id '$1' must be a reverse-DNS bundle id like com.example.app (got: $1)"
}

# Escape a string for a BRE replacement pattern (the sed delimiter is /, and
# names and ids cannot contain /).
bre_escape() {
    printf '%s' "$1" | sed -e 's/[][\.*/^$]/\\&/g'
}

# Every candidate text file, never following symlinks (node_modules holds
# symlinks into sibling repos) and never entering generated or vendored trees.
candidate_files() {
    local root="$1"
    find "$root" \
        \( -name .git -o -name target -o -name .nsh -o -name node_modules \
        -o -name .agents -o -name .zcode -o -name Cargo.lock \) -prune \
        -o -type f \( \
        -name '*.rs' -o -name '*.toml' -o -name '*.md' -o -name '*.yml' \
        -o -name '*.yaml' -o -name '*.json' -o -name '*.ts' -o -name '*.tsx' \
        -o -name '*.js' -o -name '*.sh' -o -name '*.html' -o -name '*.css' \
        -o -name '*.txt' -o -name '*.log' -o -name justfile -o -name Justfile \
        -o -name LICENSE \
        \) -print
}

rename_tree() {
    local root="$1" new_name="$2" new_id="$3"
    local old_name_v old_id_v old_snake new_snake
    old_name_v="$(old_name "$root")"
    old_id_v="$(old_id "$root")"
    [ -n "$old_name_v" ] && [ -n "$old_id_v" ] || die "could not read name/id from $root/nsh.json"
    old_snake="${old_name_v//-/_}"
    new_snake="${new_name//-/_}"

    if [ "$old_name_v" = "$new_name" ] && [ "$old_id_v" = "$new_id" ]; then
        die "the app is already named $new_name ($new_id)"
    fi

    local pat_id pat_name pat_snake
    pat_id="$(bre_escape "$old_id_v")"
    pat_name="$(bre_escape "$old_name_v")"
    pat_snake="$(bre_escape "$old_snake")"

    local files
    files="$(candidate_files "$root" | tr '\n' '\0' |
        xargs -0 grep -l -e "$old_id_v" -e "$old_name_v" -e "$old_snake" 2>/dev/null || true)"
    if [ -z "$files" ]; then
        die "no files mention $old_name_v / $old_id_v; refusing to rename nothing"
    fi

    # LC_ALL=C: the patterns are ASCII; the trees they pass through are not
    # (zh-CN.yml). The id goes first because it embeds the bare name without
    # separators.
    local f
    while IFS= read -r f; do
        [ -n "$f" ] || continue
        LC_ALL=C sed -i '' \
            -e "s/$pat_id/$new_id/g" \
            -e "s/$pat_name/$new_name/g" \
            -e "s/$pat_snake/$new_snake/g" \
            "$f"
    done <<<"$files"

    echo "Renamed $old_name_v ($old_id_v) -> $new_name ($new_id) in:"
    echo "$files" | sed "s|^$root/||"
    echo "Cargo.lock refreshes on the next cargo command."
}

roundtrip() {
    local root new_name="${1:-}" new_id="${2:-}"
    root="$(repo_root)"
    new_name="${new_name:-renamed-app}"
    new_id="${new_id:-com.example.renamedapp}"
    check_name "$new_name"
    check_id "$new_id"

    # A sibling of the repo, so `path = "../nsh-query/..."` keeps resolving.
    COPY_DIR="$(mktemp -d "$(dirname "$root")/rename-roundtrip.XXXXXXXX")"

    rsync -a \
        --exclude 'target/' \
        --exclude '.nsh/' \
        --exclude '.git/' \
        "$root/" "$COPY_DIR/"

    echo "==> Renamed copy at $COPY_DIR"
    "$COPY_DIR/scripts/rename.sh" "$new_name" "$new_id"

    echo "==> Criterion-1 gate in the copy"
    # `nsh test` re-runs the copy's cargo test before its TSX tests (the
    # tool's own order); both legs of criterion 1 are still invoked.
    local ok=no
    if (cd "$COPY_DIR" &&
        cargo fmt --all -- --check &&
        cargo clippy --all-targets -- -D warnings &&
        cargo test &&
        nsh test); then
        ok=yes
    fi

    if [ "$ok" = yes ]; then
        echo "PASS: the renamed copy passes fmt, clippy, cargo test and nsh test."
    else
        echo "FAIL: the renamed copy did not pass the criterion-1 gate." >&2
        exit 1
    fi
}

main() {
    local root
    root="$(repo_root)"
    [ -f "$root/nsh.json" ] || die "no nsh.json at $root"

    case "${1:-}" in
        --roundtrip-test)
            shift
            [ $# -le 2 ] || die "usage: scripts/rename.sh --roundtrip-test [NAME] [ID]"
            roundtrip "$@"
            ;;
        -h | --help)
            sed -n '2,16p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
            ;;
        '')
            die "usage: scripts/rename.sh <new-name> <new-id> | scripts/rename.sh --roundtrip-test [NAME] [ID]"
            ;;
        *)
            [ $# -eq 2 ] || die "usage: scripts/rename.sh <new-name> <new-id>"
            check_name "$1"
            check_id "$2"
            rename_tree "$root" "$1" "$2"
            ;;
    esac
}

main "$@"
