#!/bin/sh
# Lay out an installed runbook under one prefix: the binary, the programs it starts and the files it reads.
#
#   scripts/install.sh --prefix <dir> [--from-built debug|release]
#
# See usage() below. The tests are runbook/tests/install/ (task test:runbook; the release build: task test:install).
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)

usage() {
    cat <<'EOF'
usage: scripts/install.sh --prefix <dir> [--from-built debug|release]

Builds runbook (runbook/), the programs it starts (driver/, watch/) with `cargo build --locked --release`
and writes them under <dir>:

  <dir>/bin/runbook
  <dir>/libexec/<helper>        one file per helper program
  <dir>/share/runbook/          protocol/ bindings/ instructions/ views/ cycles/ scripts/ tools/ ui/
                                driver/examples/sdc/ VERSION MANIFEST

<dir> must be an absolute path to a directory that exists. It is not / and not inside ~/.cargo.
Nothing else is written (the build's own target/ directories aside) and no shell profile is changed.

  --prefix <dir>          where to install
  --from-built <profile>  do not build: take the binaries that exist already in runbook/target/<profile>,
                          driver/target/<profile> and watch/target/<profile> (debug or release);
                          fails with the name of the first one that is missing
  --help                  print this text

A second run over the same prefix replaces the files of the first and removes the files the old
share/runbook/MANIFEST lists that the new one does not. It removes nothing else.
EOF
}

die() {
    echo "install.sh: $*" >&2
    exit 1
}

refuse() {
    echo "install.sh: $*" >&2
    echo "usage: scripts/install.sh --prefix <dir> [--from-built debug|release] (--help says more)" >&2
    exit 2
}

# The programs of driver/ a run starts, one per line (bindings/tools, `runbook run`, `runbook doctor`).
DRIVER_HELPERS='
sdc-answers
sdc-bugfix
sdc-check-routes
sdc-conform
sdc-conform-adapter
sdc-decision-request
sdc-findings
sdc-node
sdc-replay
sdc-reply
'
# The program of watch/ a run starts.
WATCH_HELPERS='
sdc-watch
'
# The directories of the repository that runbook reads, relative to the repository (assets.rs: LAYOUT, and tools/,
# which every run reads its autonomy table from).
ASSET_DIRS='
protocol
scripts
driver/examples/sdc
bindings
instructions
views
cycles
tools
ui/web
'

prefix=
from_built=
while [ $# -gt 0 ]; do
    case $1 in
    --help | -h)
        usage
        exit 0
        ;;
    --prefix)
        [ $# -ge 2 ] || refuse "--prefix needs a directory"
        prefix=$2
        shift 2
        ;;
    --from-built)
        [ $# -ge 2 ] || refuse "--from-built needs debug or release"
        from_built=$2
        shift 2
        ;;
    *) refuse "unknown argument: $1" ;;
    esac
done

[ -n "$prefix" ] || refuse "--prefix <dir> is required"
case $from_built in
'' | debug | release) ;;
*) refuse "--from-built takes debug or release, not $from_built" ;;
esac
case $prefix in
/*) ;;
*) refuse "the prefix must be an absolute path, not $prefix" ;;
esac

# The name of a directory with `/` and `..` taken out, without asking the file system, so that a link does not hide it.
lexical() {
    printf '%s\n' "$1" | awk -F/ '
        { n = 0
          for (i = 1; i <= NF; i++) {
              if ($i == "" || $i == ".") continue
              if ($i == "..") { if (n > 0) n--; continue }
              part[++n] = $i
          }
          out = ""
          for (i = 1; i <= n; i++) out = out "/" part[i]
          print (out == "" ? "/" : out) }'
}

inside() {
    case $1 in
    "$2" | "$2"/*) return 0 ;;
    esac
    return 1
}

lexical_prefix=$(lexical "$prefix")
[ "$lexical_prefix" != / ] || refuse "the prefix is the root directory: name a directory of your own"
[ -d "$prefix" ] || refuse "the prefix $prefix does not exist as a directory: make it first"
physical_prefix=$(cd "$prefix" && pwd -P)
[ "$physical_prefix" != / ] || refuse "the prefix is the root directory: name a directory of your own"
if [ -n "${HOME:-}" ]; then
    home_lexical=$(lexical "$HOME")
    home_physical=$(cd "$HOME" 2>/dev/null && pwd -P || echo "$home_lexical")
    for cargo_dir in "$home_lexical/.cargo" "$home_physical/.cargo"; do
        for candidate in "$lexical_prefix" "$physical_prefix"; do
            if inside "$candidate" "$cargo_dir"; then
                refuse "the prefix $prefix is inside $cargo_dir: this script installs nothing into ~/.cargo"
            fi
        done
    done
    cargo_physical=$(cd "$home_physical/.cargo" 2>/dev/null && pwd -P || echo "$home_physical/.cargo")
    if inside "$physical_prefix" "$cargo_physical"; then
        refuse "the prefix $prefix is inside $cargo_physical: this script installs nothing into ~/.cargo"
    fi
fi
prefix=$physical_prefix

# Every `sdc-*` program is a name on a line of the lists above.
list() {
    printf '%s' "$1" | sed '/^$/d'
}

built_path() {
    case $1 in
    runbook) echo "runbook/target/$2/runbook" ;;
    sdc-watch) echo "watch/target/$2/sdc-watch" ;;
    *) echo "driver/target/$2/$1" ;;
    esac
}

all_programs() {
    echo runbook
    list "$DRIVER_HELPERS"
    list "$WATCH_HELPERS"
}

profile=release
if [ -n "$from_built" ]; then
    profile=$from_built
    for name in $(all_programs); do
        file="$root/$(built_path "$name" "$profile")"
        [ -f "$file" ] && [ -x "$file" ] || die "$(built_path "$name" "$profile") is missing (--from-built $profile takes binaries that exist already)"
    done
else
    unset CARGO_TARGET_DIR
    driver_bins=
    for name in $(list "$DRIVER_HELPERS"); do
        driver_bins="$driver_bins --bin $name"
    done
    echo "install.sh: building watch/ (release)" >&2
    cargo build --locked --release --manifest-path "$root/watch/Cargo.toml" --bin sdc-watch
    echo "install.sh: building driver/ (release)" >&2
    # shellcheck disable=SC2086
    cargo build --locked --release --manifest-path "$root/driver/Cargo.toml" $driver_bins
    echo "install.sh: building runbook/ (release)" >&2
    cargo build --locked --release --manifest-path "$root/runbook/Cargo.toml" --bin runbook
    for name in $(all_programs); do
        file="$root/$(built_path "$name" release)"
        [ -f "$file" ] && [ -x "$file" ] || die "the build did not make $(built_path "$name" release)"
    done
fi

# The files of the repository to copy: the directories above, minus what a run or a build left in them
# (dot-directories, runs/, target/, node_modules/) and any login or settings file, and the page's own files.
# One line per file, `<directory><TAB><name>`, sorted, so that the files of one directory are together.
tab=$(printf '\t')
sources=$(
    cd "$root"
    for dir in $(list "$ASSET_DIRS"); do
        [ -d "$dir" ] || die "the repository has no $dir/ (looked in $root)"
        find "$dir" \( -type d \( -name '.*' -o -name runs -o -name target -o -name node_modules \) -prune \) \
            -o \( \( -type f -o -type l \) ! -name '.env*' ! -iname '*credential*' ! -name 'settings.local.json' -print \)
    done
    for page in ui/*.mjs ui/package.json; do
        [ -f "$page" ] || continue
        case $page in
        ui/playwright*) ;;
        *) echo "$page" ;;
        esac
    done
) || exit 1
sources=$(printf '%s\n' "$sources" | sed "s|/\([^/]*\)\$|$tab\1|" | LC_ALL=C sort -u)

share=$prefix/share/runbook

# A path below the prefix that goes through no symbolic link: every directory above it is a real directory.
clean_path() {
    rel=$1
    case $rel in
    '' | /* | .. | ../* | */.. | */../* | ./* | */./* | */. | .) return 1 ;;
    esac
    walked=
    old_ifs=$IFS
    IFS=/
    # shellcheck disable=SC2086
    set -- ${rel%/*}
    IFS=$old_ifs
    for part in "$@"; do
        [ "$part" = . ] && continue
        walked=$walked/$part
        [ -L "$prefix$walked" ] && return 1
    done
    return 0
}

# Every file this install writes, relative to the prefix, sorted.
new_manifest=$(
    {
        echo bin/runbook
        for name in $(list "$DRIVER_HELPERS") $(list "$WATCH_HELPERS"); do
            echo "libexec/$name"
        done
        printf '%s\n' "$sources" | tr "$tab" / | sed 's|^|share/runbook/|'
        echo share/runbook/VERSION
        echo share/runbook/MANIFEST
    } | LC_ALL=C sort -u
)

for top in bin libexec share share/runbook; do
    if [ -L "$prefix/$top" ]; then
        die "$prefix/$top is a symbolic link: this script writes no file through a link"
    fi
done

if [ -f "$share/MANIFEST" ] && [ ! -L "$share/MANIFEST" ]; then
    new_lines="
$new_manifest
"
    while IFS= read -r rel; do
        [ -n "$rel" ] || continue
        case $new_lines in
        *"
$rel
"*) continue ;;
        esac
        case $rel in
        bin/* | libexec/* | share/runbook/*) ;;
        *)
            echo "install.sh: not removing $rel: outside bin, libexec and share/runbook" >&2
            continue
            ;;
        esac
        clean_path "$rel" || {
            echo "install.sh: not removing $rel: it leaves the prefix or goes through a symbolic link" >&2
            continue
        }
        if [ -f "$prefix/$rel" ] || [ -L "$prefix/$rel" ]; then
            rm -f "$prefix/$rel"
            echo "install.sh: removed $rel (listed by the old MANIFEST, not written now)" >&2
            parent=${rel%/*}
            while [ "$parent" != bin ] && [ "$parent" != libexec ] && [ "$parent" != share/runbook ]; do
                rmdir "$prefix/$parent" 2>/dev/null || break
                parent=${parent%/*}
            done
        fi
    done < "$share/MANIFEST"
fi

# `make_dir <directory relative to the prefix>`: makes it, and refuses a symbolic link or an odd name on the way.
make_dir() {
    clean_path "$1/x" || die "$1: a symbolic link or an odd name on the way: nothing is written through a link"
    mkdir -p "$prefix/$1"
}

# `clear_name <directory> <name>`: a link of an old install at the place of a file is removed (it is never
# written through), and a directory at that place is refused.
clear_name() {
    if [ -L "$prefix/$1/$2" ]; then
        rm -f "$prefix/$1/$2"
    elif [ -d "$prefix/$1/$2" ]; then
        die "$prefix/$1/$2 is a directory: cannot write the file $1/$2 there"
    fi
}

# `put_dir <directory relative to the prefix> <source file>...`: the files (absolute paths) go into the directory
# under their own names, one cp for all of them; a file of an old install is overwritten.
put_dir() {
    pd_dir=$1
    shift
    make_dir "$pd_dir"
    for pd_file in "$@"; do
        clear_name "$pd_dir" "${pd_file##*/}"
    done
    cp -pf "$@" "$prefix/$pd_dir/"
}

# `put_link <link of the repository> <directory> <name>`: all of them are relative and stay inside the copied
# directories.
put_link() {
    pl_target=$(readlink "$1")
    case $pl_target in
    /*) die "$1 is a link to the absolute path $pl_target: it cannot be copied" ;;
    esac
    make_dir "$2"
    clear_name "$2" "$3"
    rm -f "$prefix/$2/$3"
    ln -s "$pl_target" "$prefix/$2/$3"
}

mkdir -p "$prefix/bin" "$prefix/libexec" "$share"
put_dir bin "$root/$(built_path runbook "$profile")"
set --
for helper in $(list "$DRIVER_HELPERS") $(list "$WATCH_HELPERS"); do
    set -- "$@" "$root/$(built_path "$helper" "$profile")"
done
put_dir libexec "$@"

copy_files() {
    cf_dir=
    set --
    while IFS="$tab" read -r src_dir src_name; do
        if [ "$src_dir" != "$cf_dir" ]; then
            [ $# -eq 0 ] || put_dir "share/runbook/$cf_dir" "$@"
            set --
            cf_dir=$src_dir
        fi
        if [ -L "$root/$src_dir/$src_name" ]; then
            put_link "$root/$src_dir/$src_name" "share/runbook/$src_dir" "$src_name"
        else
            set -- "$@" "$root/$src_dir/$src_name"
        fi
    done
    [ $# -eq 0 ] || put_dir "share/runbook/$cf_dir" "$@"
}
printf '%s\n' "$sources" | copy_files

rm -f "$share/VERSION.new" "$share/MANIFEST.new"
"$prefix/bin/runbook" --version > "$share/VERSION.new" || die "the installed runbook does not print its version"
rm -f "$share/VERSION"
mv "$share/VERSION.new" "$share/VERSION"
printf '%s\n' "$new_manifest" > "$share/MANIFEST.new"
rm -f "$share/MANIFEST"
mv "$share/MANIFEST.new" "$share/MANIFEST"

count=$(printf '%s\n' "$new_manifest" | wc -l | tr -d ' ')
echo "install.sh: installed $count files under $prefix ($(cat "$share/VERSION"))" >&2
cat <<EOF

To put runbook on your PATH, add the line for your shell to its profile
(this script changes no profile):

  bash (~/.bashrc):  export PATH="$prefix/bin:\$PATH"
  zsh (~/.zshrc):    export PATH="$prefix/bin:\$PATH"
  fish (config.fish): fish_add_path $prefix/bin
EOF
