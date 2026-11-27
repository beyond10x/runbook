// The exchange the race specs make: `exchange <count> <a> <b>` swaps the two names in one step with
// renameat2(RENAME_EXCHANGE), `count` times, or until it fails when `count` is 0. Not a spec and not a product: a
// test helper that exchange.mjs builds once per digest of this file into ui/target/exchange/. It replaces a loop of
// `mv --exchange`, which GNU coreutils has only from 9.5 on. A failed exchange prints one line naming the call and the
// error and exits 1, so a host whose kernel or file system cannot exchange fails the case at once.
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint};
use std::process::ExitCode;

const AT_FDCWD: c_int = -100;
const RENAME_EXCHANGE: c_uint = 2;

extern "C" {
    fn renameat2(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_uint,
    ) -> c_int;
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(count), Some(a), Some(b)) = (
        args.get(1).and_then(|n| n.parse::<u64>().ok()),
        args.get(2),
        args.get(3),
    ) else {
        eprintln!("usage: exchange <count, 0 for no end> <a> <b>");
        return ExitCode::from(2);
    };
    let (Ok(ca), Ok(cb)) = (CString::new(a.as_str()), CString::new(b.as_str())) else {
        eprintln!("exchange: a name holds a NUL byte");
        return ExitCode::from(2);
    };
    let mut done: u64 = 0;
    while count == 0 || done < count {
        // SAFETY: both pointers are NUL-terminated strings that live until the call returns.
        let r = unsafe { renameat2(AT_FDCWD, ca.as_ptr(), AT_FDCWD, cb.as_ptr(), RENAME_EXCHANGE) };
        if r != 0 {
            let e = std::io::Error::last_os_error();
            eprintln!("exchange: renameat2({a}, {b}, RENAME_EXCHANGE) failed after {done} exchanges: {e}");
            return ExitCode::from(1);
        }
        done += 1;
    }
    ExitCode::SUCCESS
}
