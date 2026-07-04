//! Freelens - Open-source Kubernetes IDE (fork of Lens)
//!
//! Freelens is a free, open-source Kubernetes IDE that provides a graphical
//! interface for managing Kubernetes clusters. It's a community fork of the
//! original Lens IDE.
//!
//! Homepage: https://github.com/freelensapp/freelens

use crate::define_tool;

define_tool!(FREELENS, {
    command: "freelens",
    repo: "freelensapp/freelens",
    macos: { cask: "freelens" },
    linux: { brew: "freelens" },
    windows: { winget: "freelensapp.Freelens" },
});
