//! change.implement as one request (story:implement-one-request; `SDC_IMPLEMENT_MODE=request`,
//! docs/driver.md "Implement as one request").
//!
//! - `assemble`: the node's whole input in one prompt; the node reads nothing.
//! - `select_tree`: which files of the product tree are inlined (the input rule, below).
//! - `check_reply`: the reply's envelope checked against `schema`; `confine` and `write_files`:
//!   the files written into the call's `out/impl/`, never outside it.
//! - `run`: the requests of one implement call: the first, at most `MAX_RETRIES` retries with the
//!   check's error, at most `MAX_CONTINUATIONS` continuations by rule; never a tool loop.
//!
//! The input rule (`select_tree`): candidates are the files of the product tree (`impl/`), less
//! build output (`target/`) and hidden entries (`.sdc`, `.derived-from`, `.rework`). A file whose
//! text is not UTF-8 is listed, never inlined. Each candidate gets a rank: 0 its path is named in
//! the story's text; 1 this story's change: absent from or different in `mainline/` (the tree the
//! epic's earlier stories merged), which for a first story (no `mainline/`) is the whole tree;
//! 2 a crate root file (`ROOT_FILES`); 3 a file earlier stories merged and this story has not
//! touched. In rank and path order a file is inlined while the inlined bytes stay within the cap

mod assemble;
mod reply;
mod run;
mod tree;
#[cfg(test)]
mod tests;

pub use assemble::{assemble, pack, Assembly, Inputs};
pub use reply::{check_reply, schema, Reply};
pub use run::{run, RunArgs};
pub use tree::{confine, select_tree, write_files, Selection, TreeFile};


/// Bytes of file text inlined per request; the rest of the tree is listed by path and size.
pub const INLINE_CAP: usize = 64_000;
/// Requests after an invalid reply, per implement call (AGENTS.md: at most two retries).
pub const MAX_RETRIES: usize = 2;
/// Continuation requests per implement call, each for the paths a valid reply left `remaining`.
pub const MAX_CONTINUATIONS: usize = 2;
/// The crate's root files: inlined after the story's own files.
pub const ROOT_FILES: [&str; 5] = ["Cargo.toml", "src/lib.rs", "src/main.rs", "README.md", "Taskfile.yml"];
