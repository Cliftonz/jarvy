Jarvy · Rust-based Dev Environment CLI

Jarvy is a fast,
Rust-based command-line tool
that standardizes and automates local development environment setup for your application repositories.
It tackles the infamous “works on my machine”
problem by ensuring every developer on the team has the same development environment configuration,
regardless of their OS.
Instead of relying on remote dev environments like dev-pods or GitHub Codespaces, Jarvy runs on your local machine,
using a simple config file (jarvy.toml) in your repo to provision all required tools.
This approach is particularly useful for team leads and architects
who want to enforce consistent setups across the team. 

Jarvy makes it easy to define a standard dev stack once and apply it everywhere.

Why Jarvy?

Modern development teams face challenges with environment drift and onboarding new developers.
Jarvy was created to simplify and streamline this process.
Here is why you might want Jarvy in your workflow:
•	Standardized Environments:
Jarvy ensures all developers work in an identical environment with the same tool versions,
no matter if they use macOS, Linux, or Windows ￼.
By codifying the setup in jarvy.toml,
you eliminate cross-OS discrepancies and those hard-to-diagnose errors from mismatched setups ￼.
•	Instant Onboarding: New team members can get up and running in seconds rather than days.
Instead of following lengthy setup guides,
a developer can simply clone the repo and run jarvy to have all necessary tools installed and configured  ￼.
This drastically cuts down onboarding time and frustration.
•	Dev Environment as Code: Jarvy treats your development environment configuration as source code.
The jarvy.toml file lives in your repository (version-controlled),
providing a single source of truth for required tools and versions.
This creates a consistent developer experience and captures environment knowledge in code ￼ –
no more wiki pages of manual setup steps.
•	Cross-Platform Automation: Jarvy works on macOS, Linux, and Windows and intelligently adapts to each platform ￼.
It leverages native package managers and installers to set up tools
(e.g. Homebrew on macOS, apt on Linux, Chocolatey/Winget on Windows)
so that the same config yields the same results everywhere ￼ ￼.
Your whole team (and CI pipelines) can share one config file regardless of OS.
•	Local, Offline-Friendly:
Unlike cloud-based dev environments (such as Codespaces or DevPod)
that require internet access and remote VMs/containers,
Jarvy configures your local machine.
After the initial installations, you can work offline and at native performance.
There is no heavy container overhead or vendor lock-in – you have full control of your workstation.
•	Safe & Idempotent: Running Jarvy is safe to do repeatedly.
It will detect if a required tool is already installed (and at the correct version) and skip or update it as needed.
This means you can include jarvy in routine setup scripts or CI jobs to continually ensure consistency.
Jarvy acts as a lightweight provisioner that brings a machine to the desired state defined in jarvy.toml.
•	Extensible and Open Source: Jarvy is open source (MIT-licensed) and built with Rust for performance and reliability.
It’s designed to be extensible – if your project needs a new tool or custom setup, you can extend Jarvy’s functionality.
We welcome contributions from the community to add support for more tools, package managers, and integrations.

Installation

Jarvy is distributed as a standalone binary, so it is easy to install on any platform:
•	With Cargo (Rust): If you have Rust installed, you can install Jarvy from crates.io:

cargo install jarvy

This will compile and install the jarvy binary to your Cargo bin directory.

	•	Homebrew (macOS/Linux): Coming soon: Once a Homebrew formula is available, you will be able to install via brew install jarvy. (Placeholder badge above shows macOS/Linux support.)
	•	Download Binary: You can grab a pre-compiled binary for your OS from the GitHub Releases page. Download the release for Windows (.exe), macOS, or Linux, then add it to your PATH.

After installation, verify it is working by checking the version or help:

$ jarvy --help
Jarvy X.Y.Z - Standardize and automate local dev environment setup

USAGE:
jarvy [OPTIONS] [COMMAND]

... (help output) ...

(The exact version number and help output will depend on the latest release.)

Quick Start for Users

Using Jarvy in a project is straightforward. Follow these steps to standardize your development environment:

1. Define your environment in jarvy.toml. In the root of your application repository, add a file named jarvy.toml. This file lists all the tools, languages, and services your project needs, along with required versions. For example:

# jarvy.toml - Example configuration

[tools]
node = "18.16.0"       # Node.js version required for this project
rust = "stable"        # Rust toolchain (latest stable release)
docker = "*"           # Docker (install if not present, any version is fine)
awscli = "2.x"         # AWS CLI v2 (any 2.x version)
terraform = "1.5.3"    # Terraform v1.5.3 for infrastructure as code

In this example, the project needs Node.js 18.16.0, the Rust stable toolchain, Docker, AWS CLI v2, and Terraform 1.5.3. You can specify exact versions or use ranges/wildcards ("*", "2.x", etc.) to allow any compatible version. The [tools] table in jarvy.toml is where you list each dependency by name and the version you require. (Future versions of Jarvy may support additional configuration sections for environment variables, paths, or tasks.)

2. Run Jarvy in your project directory. Once jarvy.toml is in place, developers (and CI systems) can run the Jarvy CLI to set up the environment. Simply cd into the repository and execute:

$ jarvy
🔍 Found jarvy.toml - setting up your development environment...
✅ Node.js v18.16.0 is installed.
✅ Rust (stable) toolchain is installed.
⬇️  Docker not found, installing Docker...  
✅ Docker installed successfully.
⬇️  AWS CLI not found, installing AWS CLI v2...  
✅ AWS CLI v2 installed successfully.
⬇️  Terraform not found, installing Terraform v1.5.3...  
✅ Terraform v1.5.3 installed successfully.

All required tools are now set up! 🎉

That is it – your local development environment is now configured to match the project’s requirements. Jarvy will download and install any missing tools. If a tool or correct version is already present, Jarvy will detect it and skip the installation (reporting it as already installed). This means you can run jarvy regularly to ensure your setup stays up-to-date with the config, without harming your system.

3. Start developing! With all dependencies in place, you can run the application, tests, or other commands with confidence that everyone on the team has the same setup. New contributors can on-board quickly by using Jarvy, and you won’t hear “but it works on my machine” anymore because the environment is consistent across the board ￼.

Tip: Add jarvy.toml to your repository and mention Jarvy in your project’s README or contributor guide. This will signal to all developers to run Jarvy after cloning the repo. You might even add a one-liner in a setup script or Makefile (e.g. a make setup target) that invokes Jarvy for convenience.

How Does Jarvy Work? (Cross-Platform Support)

Jarvy is designed to work on macOS, Linux, and Windows seamlessly.
Under the hood, it detects your operating system and uses the appropriate method to install each tool:
•	macOS: Jarvy uses Homebrew for most package installations (if Homebrew is available),
since Homebrew is a widely-used package manager on macOS ￼.
For tools not in Homebrew, Jarvy can fall back to other installation methods
(like downloading a binary or using an installer).
It ensures that things like Node, Docker, etc.,
are installed as if you installed them manually, but automatically through the config.
•	Linux: Jarvy supports Debian/Ubuntu-based systems by using apt-get to install packages when possible.
(In the future it may support other distro package managers or Homebrew on Linux as needed.)
If a tool is not in the apt repositories or if you are on a different distro,
Jarvy will attempt alternate approaches such as downloading official release binaries.
The goal is that any Linux developer can get the required tools with the same single command,
without fiddling with their distro’s specifics.
•	Windows: Jarvy can run natively on Windows.
It will try to use package managers like Chocolatey or Winget to install software
(for example, installing Node.js or Docker Desktop via those managers) ￼.
If a tool is not available via a package manager, Jarvy may download the official installer or binary.
Windows development environments are often tricky to set up, but Jarvy automates the process as much as possible.
(If you use WSL2 on Windows, you can alternatively run the Linux workflow inside WSL – Jarvy supports that as well.)

Jarvy’s cross-platform logic means the same jarvy.toml config can provision an environment on any developer’s machine. This is especially useful for teams where developers use different OSes – everyone ends up with the same versions and tools. According to industry best practices, having uniform development environments leads to fewer bugs and faster onboarding ￼ ￼. Jarvy brings those benefits without requiring Docker or containers, working directly with native tooling on each OS.

(Note:
Currently Jarvy supports the most common tools and package managers out-of-the-box. If you encounter a tool
that Jarvy does not know how to install on your OS,
please check our documentation or consider contributing a new installer integration.)

Usage Tips for Developers and Architects

Jarvy is meant to be straightforward, but here are a few tips to get the most out of it:
•	Keep jarvy.toml Updated: Treat the jarvy.toml like part of your code.
Whenever your project adds a new dependency (e.g., you now require Go or a new CLI tool), update the config.
This way, Jarvy remains the up-to-date checklist of everything needed to get the project running.
•	Review on Onboarding: If you are a tech lead or architect, you can standardize dev setups by providing a jarvy.toml.
When new developers join, just have them install Jarvy and run it.
The faster they get their environment running, the faster they can be productive on the project ￼.
•	Combine with CI: You can use Jarvy in CI pipelines to ensure the build environment matches the dev environment.
For example, in a GitHub Actions workflow, you might install Jarvy and run jarvy to set up tools before running tests.
This guarantees that CI is using the same tool versions as developers, closing the “it works locally but not in CI” gap.
•	Complement vs. Replace Containers: Jarvy is not mutually exclusive with Docker or dev containers –
you can certainly still use containerized dev environments.
But Jarvy shines when you want quick,
local setups or when working on projects that do not have a full devcontainer setup.
In many cases, Jarvy can be a simpler alternative to maintaining Dockerfiles for development,
especially for projects that rely on local installations of languages and tools.

For Contributors (Building and Extending Jarvy)

We welcome contributions to make Jarvy better!
If you would like to extend Jarvy or fix a bug, here is how to get started:
•	Project Structure: Jarvy is written in Rust.
The core logic for parsing the jarvy.toml and installing tools is located in the src/ directory
(with modules for different OS installers and tool definitions).
It uses the clap crate for command-line argument parsing and serde for TOML parsing.
•	Setting up for Development: First, ensure you have Rust installed (Nightly not required; stable Rust is fine).
Fork and clone the repository, then run cargo build to compile Jarvy.
You can run the tests with cargo test.
We strive to keep the test suite comprehensive, especially for parsing config files and simulating installation logic.
•	Adding Support for a New Tool/Platform:
If you want to add a new tool that Jarvy should be able to install,
check out the existing implementations under src/tools/
(hypothetical path).
You might need to add a definition for the tool (name, possible installation methods, how to verify version)
and then implement installation logic for each OS.
For example, adding support for Python might involve using pyenv on Linux/macOS and the official installer on Windows.
We encourage discussing in an issue first if you are planning a large addition, to ensure we can integrate it smoothly.
•	Coding Style: We follow Rust Clippy and fmt guidelines.
Please run cargo fmt and cargo clippy before submitting a PR.
Write clear, concise commit messages (we use Conventional Commits for release notes, e.g., feat:
add support for Python installation).
•	Submitting a Pull Request: Once your changes are ready and tested, open a PR on GitHub.
Describe the change and link any relevant issues.
The CI pipeline will run our test suite and linters.
Maintainers will review your contribution for alignment with project goals and code quality.
We aim to be responsive and collaborative in code reviews – contributions are valued!
•	Community and Discussion: Feel free to open issues for feature requests or bug reports.
You can also join our Slack/Discord (if available) or GitHub Discussions to talk with the maintainers and other users.
We want Jarvy to solve real problems for dev teams, so feedback and ideas are very welcome.

License

This project is released under the MIT License. See the LICENSE file for details. You are free to use Jarvy in your projects – if you find it useful, we would love to hear about it!

⸻

Jarvy exists to make the lives of developers and architects easier by eliminating the friction of environment setup. With a single config file and a one-time setup, teams can achieve a repeatable, reliable development environment on any machine. We believe that setting up a new project’s dev environment should be quick and hassle-free – and with Jarvy, it finally is. Give it a try in your next project and join us in evolving how dev environments are managed. Happy coding! 🎉

GitHub Repository   •   Documentation   •   Report an Issue

---

Add a Cargo subcommand: scaffold new tools via `cargo jarvy`

Sweet — let’s add a project-specific Cargo command that scaffolds a new tool from a template, so you can do:

# creates src/tools/git.rs and wires it up
cargo jarvy new-tool git

Below is everything you need: a reusable template file and a small cargo-jarvy subcommand crate that copies the template, substitutes names, and updates src/tools/mod.rs.

Why this approach?
- Cargo treats any executable named cargo-<name> on your PATH as a subcommand, so cargo-jarvy becomes cargo jarvy ….
- If you ever prefer a full template repo flow, you can swap to cargo-generate, but a tiny local subcommand keeps it simple for this workspace.
- If you’ve seen the xtask pattern, this is the same spirit but integrated as a real Cargo subcommand.


run via workspace without installing (Recommended)
```bash

cargo run -p cargo-jarvy -- new-tool git
```
or install locally so `cargo jarvy` is available on PATH:
```bash
cargo install --path crates/cargo-jarvy

# now you can scaffold directly:
cargo jarvy new-tool docker
cargo jarvy new-tool nvm --bin nvm
```
