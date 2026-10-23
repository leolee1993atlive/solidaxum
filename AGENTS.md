# AGENTS.md — solidaxum (monorepo)

## Repository topology

This is a **single monorepo**. Everything except the gitignored reference clones lives
here and ships together.

| Directory | What it is |
|---|---|
| `solidaxum-rs/` | Rust/Axum backend — the product being built |
| `solidaxum-ui/` | SvelteKit 5 SPA frontend for the Rust backend |
| `scripts/`, `docker/`, `bench/` | tooling, packaging, benchmarks |
| `RuoYi-Vue-Plus/` | Java/Spring Boot backend — the **reference** being ported (untracked, gitignored) |
| `RuoYi-Vue-Plus-Ui/` | Vue3 frontend — the **API contract baseline** (untracked, gitignored) |
| `warm-flow/` | Java workflow engine — ported into `solidaxum-rs/crates/flow` (untracked, gitignored) |

Consequences an agent will get wrong without this:
- Cross-cutting changes (backend + frontend + docs) land in a single tree; there is no
  per-subproject repository to sync.
- Reference clones are **not** tracked. Never edit them, and never copy their license or
  data into the repo by accident.

## Project intent (drives every decision)

- `solidaxum-rs` is a Rust **port of RuoYi-Vue-Plus**, targeting feature parity.
- The **original Vue frontend's API calls are the contract baseline.** Path / HTTP method /
  response-envelope (`{code,msg,data}`, pages as `{rows,total}`) mismatches are treated as
  **real breaking bugs**, not style choices.
- When Java and Rust disagree, Java wins unless there is a documented reason. Check the
  Controller **and** the ServiceImpl — RuoYi's data-scope probes are split across both.

## Subproject guidance

- **`solidaxum-rs/` has its own `AGENTS.md`** — read it before working in the Rust backend.
  It documents the HTTP integration-test scaffold (`cargo test -p admin --features test-util`,
  SQLite in-memory + header-injected auth that bypasses Redis), the strict
  `model/repository/service/handler/route` layering, the Redis-backed auth flow, and the
  response wrapper `R<T>`.
  - **8 crates** (`core`, `system`, `admin`, `flow`, `monitor`, `job`, `dm`, `app-data`),
    edition 2024; `app-data` is the only business-side crate depending on sea-orm.
- When editing `solidaxum-rs`, prefer the commands documented in its own `AGENTS.md`
  (`cargo fmt` / `cargo clippy` / `cargo test`). Comments throughout the Rust codebase are
  in **Chinese**.

## Task-switching discipline (avoid WIP loss)

**Rule (hard, not guideline):** before starting any new task, phase, or subagent dispatch,
the working tree MUST be clean. Verify:

```bash
git status --short   # empty output = clean
```

If non-empty, choose one before proceeding:

1. **Commit** the WIP if it's a complete logical change.
2. **`git restore <paths>`** if abandoning incomplete work.
3. **`git stash push -m "WIP: <task-id>"`** if you genuinely intend to return.

"I'll come back to it" as a mental note is **not acceptable** — context loss, session
termination, and compaction all make it unreliable. If the work is real, it lives in a
commit or a stash, never in the agent's head.

## Working with the vendored sea-orm fork

`solidaxum-rs/vendor/sea-orm` is a vendored fork that adds a DM (DaMeng) backend. A few
things bite:

- **`[patch.crates-io]` must stay the last section of `solidaxum-rs/Cargo.toml`.** Every key
  after that header belongs to it; new workspace dependencies MUST be added **above** it, or
  the fork is silently dropped from the crate graph.
- Use **`git merge`, never `git rebase`**, when integrating upstream changes on lines that
  carry vendored dependencies. Re-vendoring touches thousands of files, and a rebase replays
  every re-vendor against a moving base.
- Semantic conflicts (the same logic refactored on both sides) must be read and reconciled,
  not resolved mechanically.

## Building distributable packages

`scripts/build/` produces self-contained, offline-installable packages. The product name is
resolved by `scripts/lib/repo-name.{sh,ps1}` and flows into **artifact** names (binary,
service, package, docker image) and the `@REPO_NAME@` placeholders in `docker/` and
`scripts/build/templates/`. Resolution priority: **`.repo-name` file at the repo root** (one
line — a fork rebrands the whole product by changing only this file) → git remote URL
(`origin`, then first remote) → directory name. Note the **cargo `[[bin]]` name is statically
`solidaxum`** (Cargo can't do dynamic bin names); build scripts rename it to `$REPO_NAME` at
staging (`solidaxum.service`/`solidaxum.xml` templates likewise). Existing installs keep
their old service names until reinstalled.

Run from the repo root (`dist/` receives output; it is gitignored):

| Command | Platform | Prerequisites |
|---|---|---|
| `bash scripts/build/build-linux.sh --all` | Linux host or cross-build | `cargo-zigbuild` + a `zig` on PATH (`CARGO_ZIGBUILD_ZIG_PATH` or the `ziglang` pip pkg). Pins glibc 2.28, target `x86_64-unknown-linux-gnu`. |
| `pwsh scripts/build/build-windows.ps1 -All` | Windows only | **Inno Setup 6** (`ISCC.exe` in PATH or `%ProgramFiles(x86)%\Inno Setup 6\`); also downloads WinSW on first run. |
| `bash scripts/build/build-docker.sh` | Any host with Docker | `docker` daemon (auto-re-execs with sudo if needed); bundles Redis 7-alpine. |

Common flags (bash / pwsh): `--version` / `-Version`, `--suffix` / `-Suffix` (appends a
multi-instance service-name suffix), `--force` / `-Force` (overwrite existing `dist/`
artifact). Docker also takes `--skip-host-build` if artifacts already exist.

Artifact naming (`<name>` = resolved product name): the full first-install package has no
segment (`<name>-<ver>-linux-x64.zip` / `<name>-setup-<ver>-windows-x64.exe`); field-update
zips carry a segment — `<name>-backend-<ver>-*.zip` (backend binary only; stop service,
unzip over install dir, start) and `<name>-ui-<ver>-*.zip` (SPA static files). All three can
coexist at the same version without overwriting each other. On Linux `--backend` builds the
installer package *without* `web/` (gets the `-backend` segment); `--all` folds `web/` in (no
segment). On Windows `-Backend` alone yields the lightweight backend update zip (no ISCC
needed); `-All` compiles the interactive `setup.exe` and additionally emits the ui zip.

Windows exception: the backend `setup.exe` takes its instance suffix from the **wizard**, not
the build, so one `setup.exe` installs any suffix — `-Suffix` there only names the frontend
zip (the script warns if you pass it with `-Backend`).

Each native package embeds an **interactive install wizard**: it collects port/DB/Redis,
invokes the backend's `--write-config` (renders `config.toml` from an embedded template +
CSPRNG-generates `jwt.secret`/`security.aes_key`) and `--check-config` (live-tests DB+Redis
connectivity), then registers the service and probes `/api/health`. Linux (`install.sh`) uses
systemd with a Bash TTY wizard; Windows (Inno Setup → `setup.exe`) uses an Inno Setup wizard
(WinSW service wrapper). Upgrades preserve `jwt.secret`/`security.aes_key` and hand-edited
advanced sections via `toml_edit` surgical updates. Docker (`load-and-start.sh`) remains
non-interactive (operator edits `config.toml` manually before start).

The Linux installer's pure functions (input validation, TOML raw-value passthrough,
probe-tool degradation, rollback guards) have a no-root, no-systemd unit suite — run both
files, they must not be merged (`install.sh` and `uninstall.sh` each carry a deliberate copy
of `unit_working_dir` / `list_instances`, and sourcing both would shadow one):

```bash
cd scripts/installer-payloads/backend-linux && bash test-install.sh && bash test-uninstall.sh
```

Frontend note: the Svelte UI must be an `adapter-static` SPA (build scripts assert
`build/index.html` exists and `build/server` does **not**). Frontend build =
`pnpm install --frozen-lockfile && pnpm build` inside `solidaxum-ui/`.
