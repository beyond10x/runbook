//! What `.github/workflows/release.yml` checks on the prefix and the archives (`docs/releasing.md`, "The archives").
//! Each `run:` block sets `-o pipefail`: a reader that exits at its first line or match (`grep -q`, `head`) can end its
//! writer by SIGPIPE, and the pipe then fails a step whose check held (an `nm` of `runbook`, a `tar -t` listing). The
//! alpine step compares each program's answer with the release version, and a pull request that changes what the
//! musl build reads runs the musl job.

use super::*;

fn workflow() -> String {
    fs::read_to_string(repo().join(".github/workflows/release.yml")).unwrap()
}

/// The commands after a shell pipe on one line: `||` is no pipe, and the segment before the first `|` is a writer.
fn piped_commands(line: &str) -> Vec<String> {
    let masked = line.replace("||", "\u{1}\u{1}");
    masked.split('|').skip(1).map(|s| s.replace('\u{1}', "|").trim().to_owned()).collect()
}

/// A command that may stop reading its pipe before the writer is done.
fn stops_early(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.first().copied() {
        Some("head") => true,
        Some("grep") => words[1..].iter().any(|w| {
            *w == "--quiet" || *w == "--silent" || (w.starts_with('-') && !w.starts_with("--") && w[1..].contains('q'))
        }),
        _ => false,
    }
}

#[test]
fn no_check_of_the_release_workflow_reads_a_pipe_with_a_reader_that_stops_early() {
    let text = workflow();
    let mut bad = vec![];
    for (n, line) in text.lines().enumerate() {
        for c in piped_commands(line) {
            if stops_early(&c) {
                bad.push(format!("release.yml:{}: `{}` reads a pipe and may end its writer under pipefail", n + 1, c));
            }
        }
    }
    assert!(bad.is_empty(), "write the listing to a file and read the file: {bad:#?}");
}

#[test]
fn the_rule_finds_the_readers_that_stop_early_and_leaves_the_others() {
    let found = |l: &str| piped_commands(l).iter().any(|c| stops_early(c));
    assert!(found(r#"nm "$PREFIX/bin/runbook" | grep -q -w mi_malloc_aligned || exit 1"#));
    assert!(found(r#"tar -tzf "dist/$NAME.tar.gz" | grep -qx "$NAME/bin/runbook""#));
    assert!(found(r#"x | grep -xq y"#));
    assert!(found(r#"x | grep --quiet y"#));
    assert!(found(r#"version=$(sed -n p f | head -1)"#));
    assert!(!found(r#"grep -q -w mi_malloc_aligned "$RUNNER_TEMP/runbook.nm" || exit 1"#));
    assert!(!found(r#"test -n "$pin" || { echo no; exit 1; }"#));
    assert!(!found(r#"used=$(sed -n p f | tr -d "'" | sort -u)"#));
    assert!(!found(r#"  run: |"#));
}

#[test]
fn the_alpine_step_compares_each_answer_with_the_release_version() {
    let text = workflow();
    let docker = text.lines().find(|l| l.contains("docker run")).expect("the alpine step runs docker");
    assert!(docker.contains("-e VERSION"), "the container gets the release version: {docker}");
    let pattern = text.lines().find(|l| l.contains(r#""0:${f##*/} "#)).expect("the container script matches each answer");
    assert!(pattern.contains("$VERSION"), "an answer is taken only with the release version: {pattern}");
    assert!(!text.contains("--help"), "the step runs --version only");
}

#[test]
fn a_pull_request_that_changes_what_the_musl_build_reads_runs_the_musl_job() {
    let text = workflow();
    let paths: Vec<&str> = text
        .lines()
        .skip_while(|l| l.trim() != "pull_request:")
        .skip(2)
        .take_while(|l| l.trim_start().starts_with("- "))
        .map(|l| l.trim_start().trim_start_matches("- ").trim_matches('\''))
        .collect();
    for want in [".github/workflows/release.yml", "**/Cargo.toml", "**/Cargo.lock", "rust-toolchain.toml"] {
        assert!(paths.contains(&want), "the pull_request paths name {want}: {paths:?}");
    }
}
