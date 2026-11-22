//! The exit codes of the client commands, and what an answer of the daemon maps to.
//! `docs/cli.md` holds the same table; a test compares the two.

pub const OTHER: u8 = 1;
pub const USAGE: u8 = 64;
pub const REFUSED: u8 = 65;
pub const NOT_FOUND: u8 = 66;
pub const CONFLICT: u8 = 67;
pub const UNREACHABLE: u8 = 69;
pub const DENIED: u8 = 77;
pub const CONFIG: u8 = 78;

/// Every code a client command exits with besides 0, with its name and its cause.
pub const TABLE: &[(u8, &str, &str)] = &[
    (OTHER, "failure", "any other failure: a 5xx answer, an answer that cannot be read, a failed write"),
    (USAGE, "usage", "unknown subcommand, a missing or malformed argument, no daemon named where one is needed, an https or user-info URL"),
    (REFUSED, "refused", "the daemon refused the request as invalid: HTTP 400, 413 or 422 (a refusal of the specification such as NoSuchRunbook)"),
    (NOT_FOUND, "not found", "HTTP 404, an unknown run, or a local file or directory the command names that does not exist"),
    (CONFLICT, "conflict", "HTTP 409: the state of the run or request does not take the command"),
    (UNREACHABLE, "unreachable", "no connection to the daemon: connection refused, timed out, or lost more often than a stream retries"),
    (DENIED, "denied", "HTTP 401 or 403: the token is missing or wrong"),
    (CONFIG, "config", "the token file is missing, empty or readable by group or others"),
];

/// A command that ends in a failure: the exit code and one line for standard error (empty: nothing is
/// printed, as after a closed pipe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl Failure {
    pub fn new(code: u8, message: impl Into<String>) -> Failure {
        Failure { code, message: message.into() }
    }

    pub fn usage(message: impl Into<String>) -> Failure {
        Failure::new(USAGE, message)
    }
}
