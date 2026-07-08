//! Task (go-task) - task runner / simpler Make alternative
//!
//! `task` executes targets declared in a YAML `Taskfile.yml` —
//! a widely-adopted alternative to Make with cross-platform shell
//! semantics. Upstream package name is `go-task` (brew) because
//! `task` collides with Taskwarrior in several repositories.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TASK, {
    command: "task",
    macos: { brew: "go-task" },
    // Linux: distro repos either lack it or package Taskwarrior under
    // the `task` name — Linuxbrew's `go-task` is unambiguous.
    linux: { brew: "go-task" },
    windows: { winget: "Task.Task" },
    category: "workflow",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_registration_shape() {
        assert_eq!(TASK.command, "task");
        assert_eq!(TASK.category, Some("workflow"));
        let mac = TASK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("go-task"));
        let linux = TASK.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("go-task"));
        let win = TASK.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Task.Task"));
    }
}
