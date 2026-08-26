# Porting an LNReader Source to Aidoku

This is a contributor workflow for the BaconDroid fork, not upstream policy.

## Scope and stop conditions

- Run CodeRabbit/bot waiting loops **only** on the BaconDroid fork or Tachiyomi repositories. Never start a loop on another upstream unless explicitly requested.
- Stop a review loop when a review has no actionable findings, or when a recurring finding is verified as a false positive.

## 1. Candidate gate (must pass before implementation)

- Find the LNReader plugin. Identify its source type (JSON API or HTML), every endpoint path, pagination, chapter content, deep links, filters, and required state.
- Probe live endpoints without credentials. Reject or skip the candidate if Cloudflare, a challenge, or a captcha is active without an Aidoku-compatible bypass; if rate limits are severe; or if core pages depend on fragile scraping.
- Prefer public JSON APIs. **NovelFire is a negative example:** Cloudflare detection, mostly HTML, a DataTables chapter endpoint, and rate limits make it a skip.

## 2. Prepare branches

- `origin` is the BaconDroid fork. The correct upstream for this project is `https://github.com/Aidoku-Community/sources`.
- Create the internal fork branch `add-<source>` from `origin/main` for local/fork review.
- Create the upstream branch `add-<source>-upstream` from `upstream/main`; cherry-pick only the source commits.
- For refinements, use `add-<source>-refinement` stacked on the upstream branch, then fast-forward the upstream branch when ready.
- Keep the upstream-review branch separate from the fork-internal branch.

## 3. Study references and current upstream

- Read the current `CONTRIBUTING.md` and current upstream sources. Do not rely on an older fork copy when upstream may have changed.
- Suggested references:
  - `en.novelbuddy`: JSON API, settings, notifications, and current Markdown convention.
  - `en.freewebnovel`: novel/text/deep-link patterns.
  - The source being ported: its concrete API contract.
- For chapter content:
  - If the API returns HTML, convert it to Markdown with Aidoku helpers and preserve semantics.
  - If it returns plain text, escape literal Markdown markers before returning `PageContent::text`.
  - Do not add an HTML parser for plain text.
- Use `source.json` and `filters.json` schemas from current upstream. Use `settings.json`, defaults, and `NotificationHandler` only for user-visible preferences that map to server-side behavior.

## 4. Implement minimally

- Implement listings/search, details (authors, tags, status, rating), chapter pagination/order/date parsing, chapter pages, canonical and legacy deep links, and config/filters/settings when justified.
- Match public canonical URLs and retain legacy deep links where harmless.
- Guard malformed or empty API responses and invalid chapter keys.
- Do not commit generated `package.aix`; do commit a source `Cargo.lock` when project convention requires it.

## 5. Validate

Run the exact local validation command:

```sh
nix shell nixpkgs#gcc --command bash -lc 'cargo fmt --check && cargo clippy -- -D warnings && cargo test'
```

Also run the package checks when the local CLI is available. Do not commit `package.aix`:

```sh
aidoku package && aidoku verify package.aix
```

- Verify live API behavior with focused tests.
- Require two clean local/review inspections after a fix before escalation.
- Run a Simplify pass limited to changed files; do not refactor unrelated code.

## 6. PRs and review cadence

- Check upstream `.github` for PR and issue templates. If none exists, match recent PR title/body conventions, such as:
  - Title: `feat: add en.chikari`
  - Body: `## Changes` with concise feature sections and a `Validation` section.
- Fork review cadence: wait 20 minutes, address valid comments, and repeat until two clean passes. Then wait one hour, trigger CodeRabbit with `@coderabbitai full review` for an already-reviewed commit, wait 20 minutes, and inspect.
- Apply the scope restriction from this playbook: run automated bot loops only on the BaconDroid fork or Tachiyomi repositories.
- Upstream: create the PR from the prepared upstream branch. Do not start an automated bot loop outside the allowed repositories.

## 7. Capture learnings

- After a completed port, run a concise reflection. Add only stable, repeated lessons; do not create global skills or configuration without repeated evidence.
- Chikari example: it uses a public JSON API and canonical `/series` URLs, requires plain-text Markdown escaping, supports a hidden-genre setting, and finished with all validations green.
