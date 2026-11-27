//! The runner of a run that changes a repository (`crate::handback`): every program starts with
//! [`WORKTREE_VARIABLE`] naming the run's worktree T, where the agent node of `delegation-change` works. At the level
//! `local` this is the whole of the change to the fence: the node's working directory is T instead of its call
//! directory (story `a-delegation-changes-a-repository-on-its-own-branch`, design item 7).

use crate::handback::WORKTREE_VARIABLE;
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct TreeEnv {
    inner: Arc<dyn TestRunner>,
    tree: PathBuf,
}

impl TreeEnv {
    pub fn wrap(inner: Arc<dyn TestRunner>, tree: &Path) -> Arc<dyn TestRunner> {
        Arc::new(TreeEnv { inner, tree: tree.to_path_buf() })
    }
}

impl TestRunner for TreeEnv {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let mut environment = child.environment().clone();
        environment.set.insert(WORKTREE_VARIABLE.to_owned(), self.tree.display().to_string());
        self.inner.run_child(&child.clone().with_environment(environment))
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}
