#!/bin/sh
# Lay out an installed runbook under one prefix: the binary, the programs it starts and the files it reads.
#
#   scripts/install.sh --prefix <dir> [--from-built debug|release]
#
# See usage() below. The tests are runbook/tests/install/ (task test:runbook; the slow lane: task test:install).
#
# No pathname expansion anywhere (set -f): a name read from an argument, the environment or a file is never a
# pattern. Every unquoted expansion left is a list of fixed words of this script (DRIVER_HELPERS, WATCH_HELPERS,
# ASSET_DIRS, the output of list()), each marked where it stands.
set -efu
umask 022

root=$(cd "$(dirname "$0")/.." && pwd -P)

usage() {
    cat <<'EOF'
usage: scripts/install.sh --prefix <dir> [--from-built debug|release]

Builds runbook (runbook/), the programs it starts (driver/, watch/) with `cargo build --locked --release`
and writes them under <dir>:

  <dir>/bin/runbook
  <dir>/libexec/runbook/<helper>   one file per helper program
  <dir>/share/runbook/             protocol/ bindings/ instructions/ views/ cycles/ scripts/ tools/ ui/
                                   driver/examples/sdc/ VERSION MANIFEST

<dir> must be an absolute path to a directory that exists. It is not / and not inside ~/.cargo, its name has
no newline, and it is not inside one of the directories above of the checkout.
Nothing else is written (the build's own target/ directories aside) and no shell profile is changed.

  --prefix <dir>          where to install (once)
  --from-built <profile>  do not build: take the binaries that exist already in runbook/target/<profile>,
                          driver/target/<profile> and watch/target/<profile> (debug or release);
                          fails with the name of the first one that is missing, and when the commit that
                          runbook --version prints is not HEAD of the checkout (the helpers print no
                          commit, so only runbook is checked); once
  --help                  print this text

What is copied below share/runbook/: in a git work tree the files git tracks in those directories, else the
regular files that are not dot-files; modes are 755 for a file that is executable in the checkout (git mode
100755; without git: the owner's execute bit) and 644 for the others, 755 for directories.
share/runbook/INSTALLING is there from before the first change until after VERSION and MANIFEST are written:
a prefix that has it is half installed (runbook doctor says so), and an install over it finishes the job.

A second run over the same prefix replaces the files of the first and removes the files the old
share/runbook/MANIFEST lists that the new one does not. It removes nothing else.

Exit status: 0 done, 1 failed, 2 a usage error (the arguments or the prefix are refused; nothing is written).
EOF
}

# `die` is the failure of a step; `refuse` is a refused argument or prefix (status 2, nothing written).
installing=
die() {
    echo "install.sh: $*" >&2
    if [ -n "$installing" ]; then
        echo "install.sh: the install did not finish: share/runbook/INSTALLING stays until an install over this prefix does" >&2
    fi
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
# which every run reads its autonomy table from). ui/ adds its pages: ui/*.mjs and ui/package.json.
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
seen_prefix=
seen_from_built=
while [ $# -gt 0 ]; do
    case $1 in
    --help | -h)
        usage
        exit 0
        ;;
    --prefix)
        [ $# -ge 2 ] || refuse "--prefix needs a directory"
        [ -n "$2" ] || refuse "--prefix needs a directory, not an empty value"
        [ -z "$seen_prefix" ] || refuse "--prefix is given twice"
        seen_prefix=1
        prefix=$2
        shift 2
        ;;
    --from-built)
        [ $# -ge 2 ] || refuse "--from-built needs debug or release"
        [ -n "$2" ] || refuse "--from-built needs debug or release, not an empty value"
        [ -z "$seen_from_built" ] || refuse "--from-built is given twice"
        seen_from_built=1
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
nl='
'
case $prefix in
*"$nl"*) refuse "the prefix has a newline in its name: name a directory without one" ;;
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

# Every `sdc-*` program and every directory is a name on a line of the lists above: the words are fixed.
list() {
    printf '%s' "$1" | sed '/^$/d'
}

lexical_prefix=$(lexical "$prefix")
[ "$lexical_prefix" != / ] || refuse "the prefix is the root directory: name a directory of your own"
[ -d "$prefix" ] || refuse "the prefix $prefix does not exist as a directory: make it first"
physical_prefix=$(cd "$prefix" && pwd -P)
[ "$physical_prefix" != / ] || refuse "the prefix is the root directory: name a directory of your own"
case $physical_prefix in
*"$nl"*) refuse "the prefix has a newline in its name: name a directory without one" ;;
esac
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
# A prefix inside a directory that is copied would copy itself into itself on every install.
for dir in $(list "$ASSET_DIRS") ui; do
    if inside "$physical_prefix" "$root/$dir"; then
        refuse "the prefix $prefix is inside $root/$dir: the files of that directory are copied into the prefix"
    fi
done
prefix=$physical_prefix

# Git must not be pointed at another repository by the environment.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

# Whether the checkout is the top of a git work tree of its own (an unpacked source archive inside another
# repository is not): then git says which files are the repository's.
in_git=
if command -v git >/dev/null 2>&1; then
    top=$(git -C "$root" rev-parse --show-toplevel 2>/dev/null || true)
    if [ -n "$top" ] && [ "$(cd "$top" && pwd -P)" = "$root" ]; then
        in_git=1
    fi
fi

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

tab=$(printf '\t')

# The files of the repository to copy, one line each: `<mode><TAB><path relative to the checkout>`, the mode being
# 0755, 0644 or `link`. In a git work tree: the files git tracks in the directories above, so that nothing a run,
# a build or a person left there is copied. Without git (a source archive): the regular files that are not
# dot-files and that are not below runs/, target/ or node_modules/, no link. Never `scripts/install.sh` itself, which
# no run reads, and in ui/ only the pages (ui/*.mjs, ui/package.json) and ui/web/.
collect() {
    if [ -n "$in_git" ]; then
        # shellcheck disable=SC2046
        git -C "$root" ls-files -s -z -- $(list "$ASSET_DIRS") ui | tr '\0' '\n' | awk '
            { t = index($0, "\t"); path = substr($0, t + 1); split(substr($0, 1, t - 1), m, " ")
              mode = (m[1] == "100755") ? "0755" : (m[1] == "100644") ? "0644" : (m[1] == "120000") ? "link" : ""
              if (mode == "") { print "skip\t" path; next }
              if (path == "scripts/install.sh") next
              if (path ~ /^ui\// && path !~ /^ui\/web\// && path !~ /^ui\/[^\/]*\.mjs$/ && path != "ui/package.json") next
              if (path ~ /^ui\/playwright/) next
              print mode "\t" path }'
    else
        (
            cd "$root"
            # shellcheck disable=SC2046
            for dir in $(list "$ASSET_DIRS"); do
                [ -d "$dir" ] || die "the repository has no $dir/ (looked in $root)"
                find "$dir" \( -name '.*' -o -name runs -o -name target -o -name node_modules \) -prune -o -type f ! -name '.*' -print
            done
            find ui -type d ! -path ui -prune -o -type f \( -name '*.mjs' -o -name package.json \) ! -name 'playwright*' -print
        ) | while IFS= read -r path; do
            [ "$path" != scripts/install.sh ] || continue
            if [ -x "$root/$path" ]; then
                printf '0755\t%s\n' "$path"
            else
                printf '0644\t%s\n' "$path"
            fi
        done
    fi
}

entries=$(collect) || exit 1
checked=
while IFS="$tab" read -r e_mode e_path; do
    [ -n "$e_path" ] || continue
    case $e_path in
    *"$tab"*) die "$e_path has a tab in its name: the list of files cannot hold it" ;;
    esac
    case $e_mode in
    skip)
        echo "install.sh: not copying $e_path: it is not a file or a link (a submodule?)" >&2
        continue
        ;;
    link)
        [ -L "$root/$e_path" ] || die "$e_path is tracked as a link and is not one in the work tree"
        e_target=$(readlink "$root/$e_path")
        case $e_target in
        /*) die "$e_path is a link to the absolute path $e_target: it cannot be copied" ;;
        esac
        case $(lexical "/r/${e_path%/*}/$e_target") in
        /r/*) ;;
        *) die "$e_path is a link to $e_target, which leaves the directory tree it is copied with" ;;
        esac
        ;;
    *)
        [ -f "$root/$e_path" ] && [ ! -L "$root/$e_path" ] || die "$e_path is tracked and is not in the work tree as a file"
        ;;
    esac
    checked="$checked$e_mode$tab$e_path$nl"
done <<EOF
$entries
EOF
entries=$checked
for dir in $(list "$ASSET_DIRS"); do
    case $entries in
    *"$tab$dir/"*) ;;
    *) die "the repository has no files in $dir/ (looked in $root)" ;;
    esac
done
if [ -z "$in_git" ]; then
    echo "install.sh: $root is no git work tree of its own: copying the regular files that are not dot-files" >&2
fi

# `<directory><TAB><mode><TAB><name>`, sorted, so that the files of one directory and mode are together.
sources=$(printf '%s' "$entries" | awk -F'\t' '
    { p = $2; n = match(p, /[^\/]*$/); print substr(p, 1, n - 2) "\t" $1 "\t" substr(p, n) }' | LC_ALL=C sort -u)

profile=release
if [ -n "$from_built" ]; then
    profile=$from_built
    for name in $(all_programs); do
        file="$root/$(built_path "$name" "$profile")"
        [ -f "$file" ] && [ -x "$file" ] || die "$(built_path "$name" "$profile") is missing (--from-built $profile takes binaries that exist already)"
    done
    # The helpers print no commit; runbook does, and it is the one whose build depends on the checkout's HEAD.
    built_said=$("$root/$(built_path runbook "$profile")" --version 2>/dev/null) || die "$(built_path runbook "$profile") does not print its version"
    built_commit=$(printf '%s\n' "$built_said" | sed -n 's/^.*(\([^()]*\))[[:space:]]*$/\1/p')
    head_commit=unknown
    if [ -n "$in_git" ]; then
        head_commit=$(git -C "$root" rev-parse HEAD 2>/dev/null | cut -c1-12 || true)
        [ -n "$head_commit" ] || head_commit=unknown
    fi
    [ "$built_commit" = "$head_commit" ] || die "$(built_path runbook "$profile") says commit ${built_commit:-none} ($built_said) and HEAD of $root is $head_commit: build it again"
else
    driver_bins=
    # shellcheck disable=SC2046
    for name in $(list "$DRIVER_HELPERS"); do
        driver_bins="$driver_bins --bin $name"
    done
    # `--target-dir` beats CARGO_TARGET_DIR, CARGO_BUILD_TARGET_DIR and `build.target-dir` of a configuration file,
    # so the files built here are the files taken below.
    echo "install.sh: building watch/ (release)" >&2
    cargo build --locked --release --manifest-path "$root/watch/Cargo.toml" --target-dir "$root/watch/target" --bin sdc-watch
    echo "install.sh: building driver/ (release)" >&2
    # shellcheck disable=SC2086
    cargo build --locked --release --manifest-path "$root/driver/Cargo.toml" --target-dir "$root/driver/target" $driver_bins
    echo "install.sh: building runbook/ (release)" >&2
    cargo build --locked --release --manifest-path "$root/runbook/Cargo.toml" --target-dir "$root/runbook/target" --bin runbook
    # shellcheck disable=SC2046
    for name in $(all_programs); do
        file="$root/$(built_path "$name" release)"
        [ -f "$file" ] && [ -x "$file" ] || die "the build did not make $(built_path "$name" release)"
    done
fi

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
    # Split at `/` only: the names are no patterns (set -f), whatever the old MANIFEST says.
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
        # shellcheck disable=SC2046
        for name in $(list "$DRIVER_HELPERS") $(list "$WATCH_HELPERS"); do
            echo "libexec/runbook/$name"
        done
        printf '%s' "$entries" | awk -F'\t' '{ print "share/runbook/" $2 }'
        echo share/runbook/VERSION
        echo share/runbook/MANIFEST
    } | LC_ALL=C sort -u
)

for top in bin libexec libexec/runbook share share/runbook; do
    if [ -L "$prefix/$top" ]; then
        die "$prefix/$top is a symbolic link: this script writes no file through a link"
    fi
done

# From here the prefix is half installed until the last line of the install: the marker says so.
mkdir -p "$share"
printf 'an install of this prefix was started and has not finished\n' > "$share/INSTALLING"
installing=1

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

# `put_dir <directory relative to the prefix> <mode> <source file>...`: the files (absolute paths) go into the
# directory under their own names, one cp for all of them, and get the mode (0755 or 0644, set-id bits cleared):
# the mode of the source is not carried over. A file of an old install is overwritten.
put_dir() {
    pd_dir=$1
    pd_mode=$2
    shift 2
    make_dir "$pd_dir"
    for pd_file in "$@"; do
        clear_name "$pd_dir" "${pd_file##*/}"
    done
    cp -f "$@" "$prefix/$pd_dir/"
    pd_n=$#
    while [ "$pd_n" -gt 0 ]; do
        pd_file=$1
        shift
        set -- "$@" "$prefix/$pd_dir/${pd_file##*/}"
        pd_n=$((pd_n - 1))
    done
    chmod "$pd_mode" "$@"
}

# `put_link <link of the repository> <directory> <name>`: all of them are relative and stay inside the copied
# directories (checked before the first write).
put_link() {
    pl_target=$(readlink "$1")
    make_dir "$2"
    clear_name "$2" "$3"
    rm -f "$prefix/$2/$3"
    ln -s "$pl_target" "$prefix/$2/$3"
}

mkdir -p "$prefix/bin" "$prefix/libexec/runbook"
put_dir bin 0755 "$root/$(built_path runbook "$profile")"
set --
# shellcheck disable=SC2046
for helper in $(list "$DRIVER_HELPERS") $(list "$WATCH_HELPERS"); do
    set -- "$@" "$root/$(built_path "$helper" "$profile")"
done
put_dir libexec/runbook 0755 "$@"

# `copy_group <directory> <mode> <name>...`: the files of one directory of the checkout with one mode.
copy_group() {
    cg_dir=$1
    cg_mode=$2
    shift 2
    if [ "$cg_mode" = link ]; then
        for cg_name in "$@"; do
            put_link "$root/$cg_dir/$cg_name" "share/runbook/$cg_dir" "$cg_name"
        done
        return
    fi
    for cg_name in "$@"; do
        set -- "$@" "$root/$cg_dir/$cg_name"
        shift
    done
    put_dir "share/runbook/$cg_dir" "$cg_mode" "$@"
}

copy_files() {
    cf_key=
    cf_dir=
    cf_mode=
    set --
    while IFS="$tab" read -r src_dir src_mode src_name; do
        if [ "$src_dir$tab$src_mode" != "$cf_key" ]; then
            [ $# -eq 0 ] || copy_group "$cf_dir" "$cf_mode" "$@"
            set --
            cf_key=$src_dir$tab$src_mode
            cf_dir=$src_dir
            cf_mode=$src_mode
        fi
        set -- "$@" "$src_name"
    done
    [ $# -eq 0 ] || copy_group "$cf_dir" "$cf_mode" "$@"
}
printf '%s\n' "$sources" | copy_files

rm -f "$share/VERSION.new" "$share/MANIFEST.new"
"$prefix/bin/runbook" --version > "$share/VERSION.new" || die "the installed runbook does not print its version"
rm -f "$share/VERSION"
mv "$share/VERSION.new" "$share/VERSION"
printf '%s\n' "$new_manifest" > "$share/MANIFEST.new"
rm -f "$share/MANIFEST"
mv "$share/MANIFEST.new" "$share/MANIFEST"
rm -f "$share/INSTALLING"
installing=

# `quote_dq`: the text inside double quotes for bash and zsh (`\`, `"`, `$` and a backquote escaped).
quote_dq() {
    printf '%s' "$1" | sed 's/[\\"$`]/\\&/g'
}

# `quote_fish`: the text inside single quotes for fish (`\` and `'` escaped).
quote_fish() {
    printf '%s' "$1" | sed "s/[\\\\']/\\\\&/g"
}

count=$(printf '%s\n' "$new_manifest" | wc -l | tr -d ' ')
echo "install.sh: installed $count files under $prefix ($(cat "$share/VERSION"))" >&2
printf '\nTo put runbook on your PATH, add the line for your shell to its profile\n(this script changes no profile):\n\n'
printf '  bash (~/.bashrc):  export PATH="%s/bin:$PATH"\n' "$(quote_dq "$prefix")"
printf '  zsh (~/.zshrc):    export PATH="%s/bin:$PATH"\n' "$(quote_dq "$prefix")"
printf "  fish (config.fish): fish_add_path '%s/bin'\\n" "$(quote_fish "$prefix")"
