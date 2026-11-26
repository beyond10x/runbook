//! A preload for the test step of `runbook/` (`LD_PRELOAD`): the calls that wait until a file's data is on the disk
//! return at once. The tests write the product's durable state (the daemon's journal and run stores, Loom's run store,
//! the spend ledger) into homes under `target/` that a test drops when it ends; a sync there proves nothing a test can
//! observe, since a process that is killed leaves its writes in the page cache, and it costs a disk flush each time:
//! 373 flushes in one test of an in-process run, and the step's time on the forge varied by 1.7 times with its runner's
//! disk (`docs/forge-gate.md`, "The runbook step's syncs"). The write itself, the rename and every error of an open or a
//! write are unchanged.

use std::os::raw::{c_int, c_longlong, c_uint};

/// `fsync(2)`: the file's data and metadata are not flushed; success.
#[no_mangle]
pub extern "C" fn fsync(_fd: c_int) -> c_int {
    0
}

/// `fdatasync(2)`: as [`fsync`].
#[no_mangle]
pub extern "C" fn fdatasync(_fd: c_int) -> c_int {
    0
}

/// `syncfs(2)`: as [`fsync`], for the file system of `fd`.
#[no_mangle]
pub extern "C" fn syncfs(_fd: c_int) -> c_int {
    0
}

/// `sync_file_range(2)`: as [`fsync`], for a range of the file.
#[no_mangle]
pub extern "C" fn sync_file_range(_fd: c_int, _offset: c_longlong, _nbytes: c_longlong, _flags: c_uint) -> c_int {
    0
}

/// `sync(2)`: nothing is flushed.
#[no_mangle]
pub extern "C" fn sync() {}
