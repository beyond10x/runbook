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

# What the caller's environment must not change: the field separators, what `cd`, `sort` and the globbing do, the
# start files and the strictness of the shell, and which repository, index and object store git reads.
IFS=$(printf ' \t\n_')
IFS=${IFS%_}
unset CDPATH ENV BASH_ENV GLOBIGNORE POSIXLY_CORRECT LANG LANGUAGE LC_COLLATE LC_CTYPE LC_MESSAGES
LC_ALL=C
export LC_ALL
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
    GIT_NAMESPACE GIT_CEILING_DIRECTORIES GIT_CONFIG GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS GIT_PREFIX \
    GIT_LITERAL_PATHSPECS GIT_GLOB_PATHSPECS GIT_NOGLOB_PATHSPECS GIT_ICASE_PATHSPECS GIT_QUARANTINE_PATH

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
no newline, it is not inside one of the directories above of the checkout, and it is not the checkout, a
directory that holds it, or its .git.
Nothing else is written (the build's own target/ directories aside) and no shell profile is changed.
The build is for the machine the script runs on: with a target triple set (CARGO_BUILD_TARGET, build.target of a
cargo configuration file) the script stops and says so, and installs no cross build.

  --prefix <dir>          where to install (once)
  --from-built <profile>  do not build: take the binaries that exist already in runbook/target/<profile>,
                          driver/target/<profile> and watch/target/<profile> (debug or release);
                          fails with the name of the first one that is missing, and when the commit that
                          runbook --version prints is not HEAD of the checkout (the helpers print no
                          commit, so only runbook is checked; a build made here is checked the same way); once
  --help                  print this text

What is copied below share/runbook/: when the checkout has a .git, the files git tracks in those directories (the
bytes of the work tree: VERSION ends in " dirty" when a tracked file there differs from HEAD, and names HEAD,
the commit the binary was built from); when git cannot be asked about a checkout that has a .git, the script
stops and copies nothing. Without a .git (a source archive): the regular files that are not dot-files. A link is
copied only when its target stays inside the checkout and is copied too. Modes are 755 for a file that is
executable in the checkout (git mode 100755; without git: the owner's execute bit) and 644 for the others; the
directories below libexec/runbook and share/runbook are 755, without a set-id bit.
share/runbook/INSTALLING is there from before the first change until after VERSION and MANIFEST are written:
a prefix that has it is half installed (runbook doctor says so, and a runbook started from <dir>/bin runs
nothing), and an install over it finishes the job.

A second run over the same prefix replaces the files of the first and removes the files the old
share/runbook/MANIFEST lists that the new one does not. It removes nothing else.

Limits:
  - Two installs at once into one prefix are not held apart (there is no lock): the one that ends first
    removes INSTALLING while the other still copies. Run one at a time; after a stop, run it again.
  - A prefix that is a link to a directory whose name ends in a newline is installed into the directory
    without the newline: name the directory itself, or a link to a directory without such a name.
  - A source tree without a .git unpacked inside a repository that tracks runbook/Cargo.toml is built with that
    repository's commit, and the script then stops at the commit check (an archive has no HEAD): unpack it
    outside any repository. A .git that is a file names the repository it points to; that repository's HEAD is
    the commit taken.
  - bin, libexec and share keep the mode they have (mkdir's, under a set-id prefix the set-id bit is
    inherited): they are shared with other packages. Everything below libexec/runbook and share/runbook is set.

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
# The checkout, or what holds it, is no prefix: its files would be copied into themselves or their directory replaced.
if inside "$root" "$physical_prefix"; then
    refuse "the prefix $prefix is the checkout $root or holds it: name a directory of its own"
fi
if inside "$physical_prefix" "$root/.git"; then
    refuse "the prefix $prefix is inside the git directory of the checkout"
fi
prefix=$physical_prefix

# Whether the checkout is a git work tree of its own: then git says which files are the repository's. A source
# directory with a `.git` (a directory, or a file of a linked work tree) is that, and when git cannot be asked
# about it the script stops: copying it like an archive would copy what git does not track (a settings file, a key,
# a note of a run). An archive has no `.git` at its root, also when it is unpacked inside another repository.
in_git=
if [ -e "$root/.git" ] || [ -L "$root/.git" ]; then
    command -v git >/dev/null 2>&1 || die "$root has a .git and git is not on PATH: put git on PATH (this script copies the files git tracks and nothing else), or install from a source archive that has no .git"
    top=$(git -C "$root" rev-parse --show-toplevel 2>/dev/null) || top=
    [ -n "$top" ] || die "$root has a .git and git cannot be asked about it (git refuses a repository of another owner, or the repository is broken): run this as the owner of the checkout, name it in safe.directory, or repair it, then run the script again; nothing was written"
    [ "$(cd "$top" && pwd -P)" = "$root" ] || die "$root has a .git and git says the top of its work tree is $top, not $root: nothing was written"
    in_git=1
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
            cd "$root" || exit 1
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
e_links=
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
        *"$tab"* | *"$nl"*) die "$e_path is a link whose target has a tab or a newline in it: it cannot be copied" ;;
        esac
        e_links="$e_links$e_path$tab$e_target$nl"
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

# A link is installed as it is, so its target must stay inside the checkout, name a file or directory that is copied
# too, and mean the same below the prefix: `..` only at the start of the target, and no `.` or empty part. Nothing
# is written before every link passed.
while IFS="$tab" read -r l_path l_target; do
    [ -n "$l_path" ] || continue
    case $l_target in
    '') die "$l_path is a link with an empty target" ;;
    /*) die "$l_path is a link to the absolute path $l_target: it cannot be copied" ;;
    esac
    l_rest=$l_target
    while :; do
        case $l_rest in
        ../*) l_rest=${l_rest#../} ;;
        *) break ;;
        esac
    done
    case /$l_rest/ in
    */../* | */./* | *//*) die "$l_path is a link to $l_target: .. is allowed at the start of a link target only, and no . or empty part" ;;
    esac
    l_full=$(lexical "$root/${l_path%/*}/$l_target")
    case $l_full in
    "$root"/*) ;;
    *) die "$l_path is a link to $l_target, which leaves the checkout $root" ;;
    esac
    l_rel=${l_full#"$root"/}
    case $entries in
    *"$tab$l_rel$nl"* | *"$tab$l_rel/"*) ;;
    *) die "$l_path is a link to $l_target ($l_rel), which is not among the files that are copied" ;;
    esac
done <<EOF
$e_links
EOF
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
else
    # `build_release <crate directory> <bin>...`: `cargo build --locked --release` of those programs. `--target-dir`
    # beats CARGO_TARGET_DIR, CARGO_BUILD_TARGET_DIR and `build.target-dir` of a configuration file. A target triple
    # (CARGO_BUILD_TARGET, `build.target`) writes below <target dir>/<triple>/release/, where this script does not look,
    # so it takes an older native build for the new one: the script asks cargo which files it built and stops when
    # one of them is not the file it takes. It builds for the machine it runs on, and for no other triple.
    build_release() {
        br_crate=$1
        shift
        br_names=$*
        set -- --locked --release --manifest-path "$root/$br_crate/Cargo.toml" --target-dir "$root/$br_crate/target" --message-format=json-render-diagnostics
        # shellcheck disable=SC2086
        for br_name in $br_names; do
            set -- "$@" --bin "$br_name"
        done
        mkdir -p "$root/$br_crate/target"
        br_log=$root/$br_crate/target/install-build.json
        rm -f "$br_log"
        echo "install.sh: building $br_crate/ (release)" >&2
        cargo build "$@" > "$br_log" || die "cargo build failed in $br_crate/"
        # shellcheck disable=SC2086
        for br_name in $br_names; do
            br_file="$root/$(built_path "$br_name" release)"
            grep -F -q "\"executable\":\"$br_file\"" "$br_log" || die "cargo did not report $(built_path "$br_name" release) as built: a target triple (CARGO_BUILD_TARGET, build.target of a cargo configuration file) or another setting moves the output; unset it, or build for this machine yourself and use --from-built release (this script installs no cross build)"
            [ -f "$br_file" ] && [ -x "$br_file" ] || die "the build did not make $(built_path "$br_name" release)"
        done
        rm -f "$br_log"
    }
    # shellcheck disable=SC2046
    build_release watch $(list "$WATCH_HELPERS")
    # shellcheck disable=SC2046
    build_release driver $(list "$DRIVER_HELPERS")
    build_release runbook runbook
fi

# The helpers print no commit; runbook does, and it is the one whose build depends on the checkout's HEAD. Built or
# taken as it is, its commit is HEAD of the checkout, or the script stops.
built_said=$("$root/$(built_path runbook "$profile")" --version 2>/dev/null) || die "$(built_path runbook "$profile") does not print its version"
built_commit=$(printf '%s\n' "$built_said" | sed -n 's/^.*(\([^()]*\))[[:space:]]*$/\1/p')
head_commit=unknown
if [ -n "$in_git" ]; then
    head_commit=$(git -C "$root" rev-parse HEAD 2>/dev/null | cut -c1-12 || true)
    [ -n "$head_commit" ] || head_commit=unknown
fi
[ "$built_commit" = "$head_commit" ] || die "$(built_path runbook "$profile") says commit ${built_commit:-none} ($built_said) and HEAD of $root is $head_commit: build it again"

# VERSION is what the binary prints, and ` dirty` after it when a tracked file of the copied directories differs
# from HEAD: the bytes installed are those of the work tree.
version_mark=
if [ -n "$in_git" ]; then
    # shellcheck disable=SC2046
    changed=$(GIT_OPTIONAL_LOCKS=0 git -C "$root" status --porcelain --untracked-files=no -- $(list "$ASSET_DIRS") ui) || die "git status of $root failed"
    [ -z "$changed" ] || version_mark=' dirty'
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

# `clear_name <directory> <name>`: a link of an old install at the place of a file is removed (it is never
# written through), and a directory at that place is refused.
clear_name() {
    if [ -L "$prefix/$1/$2" ]; then
        rm -f "$prefix/$1/$2"
    elif [ -d "$prefix/$1/$2" ]; then
        die "$prefix/$1/$2 is a directory: cannot write the file $1/$2 there"
    fi
}

# From here the prefix is half installed until the last line of the install: the marker says so. A link or a leftover
# at the marker's place is cleared like the place of every other name this script writes: nothing is written through it.
mkdir -p "$share"
clear_name share/runbook INSTALLING
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
        share/runbook/INSTALLING | share/runbook/VERSION | share/runbook/MANIFEST | share/runbook/VERSION.new | share/runbook/MANIFEST.new)
            echo "install.sh: not removing $rel: the install keeps that file for itself" >&2
            continue
            ;;
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
# The directories of this product's own trees (libexec/runbook and share/runbook, and below them) get 755 and no set-id
# bit, whatever the prefix hands down (a setgid prefix gives 2755) and whatever an older install left. bin, libexec and
# share are shared with other packages: they keep the mode mkdir gave them.
make_dir() {
    clean_path "$1/x" || die "$1: a symbolic link or an odd name on the way: nothing is written through a link"
    mkdir -p "$prefix/$1"
    case $1 in
    libexec/runbook | libexec/runbook/* | share/runbook | share/runbook/*) ;;
    *) return 0 ;;
    esac
    md_path=$1
    set --
    while [ "$md_path" != libexec ] && [ "$md_path" != share ]; do
        set -- "$@" "$prefix/$md_path"
        md_path=${md_path%/*}
    done
    chmod a-s,u=rwx,go=rx "$@"
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

installed_said=$("$prefix/bin/runbook" --version) || die "the installed runbook does not print its version"
rm -f "$share/VERSION.new"
printf '%s%s\n' "$installed_said" "$version_mark" > "$share/VERSION.new"
rm -f "$share/VERSION"
mv "$share/VERSION.new" "$share/VERSION"
rm -f "$share/MANIFEST.new"
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
