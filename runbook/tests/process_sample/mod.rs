//! The processes below a process, as a case samples them to judge which programs a run started. A sample walks down
//! through the `children` file of each thread (`/proc/<pid>/task/<tid>/children`): it reads the processes below the
//! root, not every process of the host.
//!
//! A process is judged by the program it runs after its `exec`. Between its fork and its `exec` a child runs its
//! parent's executable with its parent's arguments and carries the name of the thread that forked it: `runbook` below
//! the runner, `run-<id>` below a daemon run's thread, the script's name below a shell. A sample takes no name from it
//! then; the next sample judges it by the program it runs. What a child runs is read in the order the kernel's `exec`
//! writes it: the new memory and its executable first, the name next, the arguments last. So a child whose arguments
//! differ from its parent's has its new name already, and one that has an executable and shows no arguments is inside
//! its `exec` and is read again until it shows them. A child without memory is ending or ended (a zombie): it keeps the
//! name of the last program it ran, or its parent's when it never called `exec`, and is judged by that name.

use std::collections::BTreeSet;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Every process below `root`: its pid and the name of the program it runs, `None` for a child that has not called
/// `exec` yet. The processes below such a child are walked and judged too.
pub(crate) fn below(root: u32) -> Vec<(u32, Option<String>)> {
    let mut found = Vec::new();
    let mut seen: BTreeSet<u32> = BTreeSet::from([root]);
    let mut todo = vec![root];
    while let Some(pid) = todo.pop() {
        let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else { continue };
        let parent = Image::of(pid);
        for task in tasks.flatten() {
            let Ok(children) = std::fs::read_to_string(task.path().join("children")) else { continue };
            for child in children.split_whitespace().filter_map(|c| c.parse::<u32>().ok()) {
                if !seen.insert(child) {
                    continue;
                }
                let Some(program) = program(&parent, child) else { continue };
                found.push((child, program));
                todo.push(child);
            }
        }
    }
    found
}

/// The names of the programs below `root`; a child that has not called `exec` yet is left to a later sample.
pub(crate) fn programs_below(root: u32) -> Vec<String> {
    below(root).into_iter().filter_map(|(_, name)| name).collect()
}

/// What a process runs: the target of `/proc/<pid>/exe` and the bytes of `/proc/<pid>/cmdline`.
struct Image {
    exe: Option<std::path::PathBuf>,
    args: Option<Vec<u8>>,
}

impl Image {
    fn of(pid: u32) -> Image {
        // the executable before the arguments: the order in which an `exec` writes them is the reverse
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
        let args = std::fs::read(format!("/proc/{pid}/cmdline")).ok();
        Image { exe, args }
    }
}

/// How `child`, a child of the process `parent` describes, is judged: `Some(Some(name))` by the program it runs,
/// `Some(None)` not yet (it has not called `exec`), `None` when it is gone.
fn program(parent: &Image, child: u32) -> Option<Option<String>> {
    // a child inside its `exec` has its new executable and shows no arguments for the few instructions between the
    // switch of its memory and the setup of its new stack; one held there longer than this by the scheduler is judged
    // by the name it has then
    let limit = Instant::now() + Duration::from_secs(1);
    loop {
        let image = Image::of(child);
        let args = image.args.as_ref()?;
        if image.exe.is_none() {
            // no memory: the child is ending or ended, and keeps the name of what it ran last
            return name_of(child).map(Some);
        }
        if args.is_empty() && Instant::now() < limit {
            std::thread::yield_now();
            continue;
        }
        if image.exe == parent.exe && image.args == parent.args {
            return Some(None);
        }
        return name_of(child).map(Some);
    }
}

/// The name (`comm`) of process `pid` from `/proc/<pid>/stat`, or `None` when it is gone.
fn name_of(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (open, close) = (stat.find('(')?, stat.rfind(')')?);
    Some(stat[open + 1..close].to_string())
}

/// Waits up to `secs` seconds for `done`, polling every 5 ms.
fn until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(secs);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_process_held_between_its_fork_and_its_exec_is_judged_by_the_program_it_runs_after_its_exec() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    let me = std::process::id();
    let mut fds = [0; 2];
    // SAFETY: pipe2 writes two descriptors into the array it is given.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    let (read_end, write_end) = fds.into();
    // SAFETY: both descriptors were just opened by pipe2 and are owned here alone.
    let (gate, release) = unsafe { (OwnedFd::from_raw_fd(read_end), OwnedFd::from_raw_fd(write_end)) };
    // the thread that forks carries a name of its own: the forked child carries it until its exec, as a child of the
    // runner carries `runbook` and a child of the daemon's run thread `run-<id>`
    let spawner = std::thread::Builder::new()
        .name("held-fork".into())
        .spawn(move || {
            let mut command = Command::new("sleep");
            command.arg("30").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            // SAFETY: close and read are async-signal-safe and nothing is allocated between the fork and the exec.
            unsafe {
                command.pre_exec(move || {
                    libc::close(write_end);
                    let mut byte = 0u8;
                    while libc::read(read_end, (&mut byte as *mut u8).cast(), 1) < 0 && *libc::__errno_location() == libc::EINTR {}
                    Ok(())
                })
            };
            command.spawn().unwrap()
        })
        .unwrap();
    until("the forked child to show below this process", 10, || !below(me).is_empty());
    let mut judged: BTreeSet<String> = BTreeSet::new();
    for _ in 0..20 {
        judged.extend(programs_below(me));
        std::thread::sleep(Duration::from_millis(5));
    }
    let while_held = below(me);
    drop(release);
    let mut child = spawner.join().unwrap();
    until("the child to run its program", 10, || {
        judged.extend(programs_below(me));
        judged.contains("sleep")
    });
    child.kill().unwrap();
    child.wait().unwrap();
    drop(gate);
    assert!(while_held.iter().all(|(_, name)| name.is_none()), "the held child was given a program: {while_held:?}");
    assert_eq!(judged, BTreeSet::from(["sleep".to_string()]), "a process held before its exec was judged by the name it carried from its parent");
}

#[test]
fn a_child_that_runs_its_parents_program_with_arguments_of_its_own_is_judged_by_its_name() {
    use std::os::unix::process::CommandExt;
    // the outer `sh` forks a subshell, which runs `sh` again with other arguments, and that one starts `sleep`
    let mut outer = Command::new("sh").args(["-c", "sh -c 'sleep 30; :' & wait"]).process_group(0).stdin(Stdio::null()).spawn().unwrap();
    let root = outer.id();
    let mut judged: BTreeSet<String> = BTreeSet::new();
    let mut held = Vec::new();
    until("the inner sh and its sleep", 10, || {
        judged.extend(programs_below(root));
        held = below(root);
        judged.contains("sleep")
    });
    // SAFETY: a signal to the process group this case just started.
    unsafe { libc::kill(-(root as i32), libc::SIGKILL) };
    outer.wait().unwrap();
    assert_eq!(judged, BTreeSet::from(["sh".to_string(), "sleep".to_string()]), "below {held:?}");
}
