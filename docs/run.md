# Task runner (`jarvy run`)

`jarvy run` executes named shell commands from a `[commands]` table in
your `jarvy.toml` — the same idea as [npm scripts](https://docs.npmjs.com/cli/commands/npm-run-script)
(`npm run <script>`), but living next to the rest of your dev-environment
config and working for any stack.

```console
$ jarvy run build
> cargo build
   Compiling jarvy v0.6.3
    Finished `dev` profile
```

## Quick start

Add a `[commands]` table to `jarvy.toml` — each key is a name, each value
is the shell command it runs:

```toml
[commands]
build = "cargo build"
test  = "cargo test --all-features"
dev   = "docker compose up -d && cargo watch -x run"
lint  = "cargo clippy --all-features -- -D warnings"
```

Then:

```console
$ jarvy run            # list everything that's defined
$ jarvy run build      # run one by name
$ jr build             # same, after `jarvy shell-init --apply`
```

## Defining commands

- **Values are shell command lines**, run via `sh -c` on Unix and
  `cmd /C` on Windows. Chaining (`&&`, `||`), pipes, and redirects work —
  naming a command explicitly is your consent to run exactly what you
  wrote.
- **Cross-platform configs** should stick to constructs both shells
  understand, or define per-project conventions (there is no per-OS
  variant syntax in v1).
- Command names with control characters or Unicode bidi/zero-width
  characters (Trojan Source) are dropped at load time.

### Well-known names: `run`, `test`, `setup`

Three names are *well-known*: they also power the corresponding entries
in the interactive menu you get from a bare `jarvy` invocation. Defining
them makes both surfaces consistent:

```toml
[commands]
run   = "npm start"      # what "Run project" in the menu executes
test  = "npm test"       # what "Run tests" executes
setup = "jarvy setup"    # what "Set up environment" executes
```

Any other key (`build`, `dev`, `deploy`, …) is an *extra* command,
available only through `jarvy run <name>`.

### Lifecycle hooks: `pre<name>` and `post<name>`

Exactly like npm: if `pre<name>` or `post<name>` keys exist, `jarvy run
<name>` runs them around the main command:

```toml
[commands]
prebuild = "npm run clean"
build    = "npm run compile"
postbuild = "cp -r dist/ ../server/static"
```

```console
$ jarvy run build
> npm run clean
> npm run compile
> cp -r dist/ ../server/static
```

Semantics (matching npm):

- A failing `pre` hook **aborts the run** — the main command never
  starts, and the hook's exit code becomes the process exit code.
- `post` runs only after a **successful** main command; a failing
  `post` fails the run with its exit code.
- Extra `--` arguments go to the **main command only**, never to hooks.
- Hooks are ordinary `[commands]` entries — they appear in the listing
  and can be run directly (`jarvy run prebuild`).

## Running commands

### By name

```console
$ jarvy run test
> cargo test --all-features
```

The command line is printed (ANSI-stripped) before execution so you
always see what is about to run.

### Passing extra arguments

Everything after `--` is appended to the command line, each argument
quoted for the platform shell:

```console
$ jarvy run test -- --lib config::
> cargo test --all-features --lib 'config::'
```

This mirrors `npm run test -- --watch`. Two refusals protect you here:
arguments containing NUL bytes are rejected everywhere, and on Windows
arguments containing `%` are rejected because `cmd.exe` expands
`%VAR%` even inside quotes — there is no way to pass one verbatim.

### Listing

```console
$ jarvy run
Commands defined in ./jarvy.toml:

  build  cargo build
  dev    docker compose up -d && cargo watch -x run
  test   cargo test --all-features

Run one with: jarvy run <name>
```

`--format json` emits the same listing as a machine-readable envelope
for tooling:

```console
$ jarvy run --format json | jq '.commands[].name'
```

### Choosing the config file

`jarvy run` reads `./jarvy.toml` by default; point elsewhere with
`-f/--file path/to/jarvy.toml`.

## Execution semantics

| Aspect | Behavior |
|---|---|
| Working directory | The directory **containing the config file** (like npm scripts running from the package root), regardless of where you invoke from |
| Exit code | The child's exit code is propagated verbatim; a signal-killed child exits `1` |
| Unknown name | Error + exit `2` (CONFIG_ERROR) — no fallback guessing (`jarvy run test` never invents `cargo test` for you) |
| Missing/broken `jarvy.toml` | Error + exit `2` |
| stdio | Inherited — interactive commands, colors, and progress bars work |
| Environment | Inherited from your shell (jarvy adds nothing) |

## The `jr` shorthand

One command wires it up, whatever way you installed jarvy:

```console
$ jarvy shell-init --apply
Added to /home/you/.zshrc:

  eval "$(jarvy shell-init --shell zsh)"
```

Open a new shell and `jr <name>` is `jarvy run <name>`. Supported for
bash, zsh, sh, fish, PowerShell, and nushell (nushell gets a
materialized `~/.jarvy/init.nu` — re-run `--apply` after upgrading
jarvy to refresh it). The snippet also runs `jarvy ensure --quiet` on
shell start when enabled. Applying twice is a no-op.

Prefer to wire it yourself? `jarvy shell-init` (no flag) prints the
snippet for your rc file.

## Coming from npm?

| npm | jarvy |
|---|---|
| `"scripts"` in `package.json` | `[commands]` in `jarvy.toml` |
| `npm run` (list) | `jarvy run` |
| `npm run build` | `jarvy run build` |
| `npm test` / `npm start` | well-known `test` / `run` keys (also drive the interactive menu) |
| `npm run test -- --watch` | `jarvy run test -- --watch` |
| `prebuild` / `postbuild` scripts | same names, same semantics |
| `npm run env` | not supported — commands inherit your shell environment untouched |

## Telemetry

When telemetry is enabled, `run.command.*` events carry the command
*name* (display-sanitized) and a truncated hash — never the command
text itself. See [Telemetry](telemetry.md).
