# Aidoku Community Sources — AI Agent Guide

## Project Overview

Community-maintained Aidoku sources. Each source is a Rust crate compiled to
`wasm32-unknown-unknown` using the Aidoku SDK (`aidoku-rs`).

## Repo Conventions & Session Isolation

### Upstream source for testing

- Always use the **Next** SDK version and the **main** branch of the upstream
  repo (`https://github.com/Aidoku-Community/sources.git`). Never use legacy SDK
  or a fork branch for validation.
- Fetch upstream `main` and use it as the base for all work.

### Branch workflow

1. **Project branch**: `fix/novelbuddy-empty-chapters` — long-lived branch on
   the fork (`origin`). This is where finished work lands.
2. **Temp branch**: For each work session, create a temporary branch off
   `upstream/main` (e.g. `tmp/diag-novelbuddy-runtime`). Do all work here.
3. **Merge**: When work is complete, merge the temp branch into the project
   branch.
4. **Cleanup**: Delete the temp branch after merging (`git branch -D <temp>`).
5. **Push**: Push the project branch to `origin` (the fork). Never push to
   `upstream`.

### Throwaway clones

- For one-off builds/tests, clone into `/tmp/opencode/` and clean up when done.
- Work in detached HEAD or temp branches. Do not create branches on other
  sessions' repos.

### Plans

- All plans should go in `~/Plans/` (outside of `~/Projects/`), with one subfolder per project (e.g. `~/Plans/novelbuddy-empty-chapters/`).

## Build

```sh
# Build a single source to WASM
cd sources/<source-id>
cargo build --release --target wasm32-unknown-unknown
```

## Testing

```sh
# Run unit tests (requires aidoku-test runtime)
cd sources/<source-id>
cargo test --release --target wasm32-unknown-unknown
```

## Language

- **Respond to the user in French.**
- **Write code, docs, and comments in English.**
