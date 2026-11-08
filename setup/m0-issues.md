# Milestone 0 issues

Create each as a GitHub issue in milestone `M0 — Groundwork and kernel evaluation`, with the
labels shown. The text under each heading is the issue body. Keep the numbering in the title
(`M0-01 …`) so they sort in order.

---

## M0-01 Owner approval through the "owner-approved" label
**Labels:** `agent-ready`, `area: infra` · *after M0-04*

**Background:** agents work through the owner's own GitHub account (decision 0005), so GitHub's
normal "approve" button can't be used: nobody can approve their own pull request. The owner's
approval is given by adding the label `owner-approved` to a pull request instead.

**Do:**
- Create the label `owner-approved` (colour `0E8A16`, description "Approved by the project owner").
- Add a required check that **fails** without `owner-approved` only when a pull request:
  - **modifies or deletes an existing file** in a folder listed in `.github/CODEOWNERS`
    (tests, references, goldens) — *adding new files there does not need approval*;
  - adds or changes anything in `docs/specs/`;
  - deletes a test, adds `#[ignore]`, or loosens a tolerance anywhere, including tests inside
    source files (shared with M0-06).
  Every other pull request passes this check automatically. It must re-run when labels change.
- **Goal: the owner is asked rarely.** Report in the weekly status how many pull requests needed
  his approval, so the rule can be adjusted if it's too frequent.
- Make it a required status check on `main`.
- When a pull request needs approval, the agent sends the owner a short message: what changed, why,
  a picture if visible, and the link. It then waits.

**Acceptance:** a pull request that only adds new tests merges without the label; one that edits an
existing test can't be merged until the label is added; adding
the label in the GitHub app makes it mergeable; agents' permission rules block adding the label.

---

## M0-02 Set up a Dev Drive and the Rust toolchain on the Windows machine
**Labels:** `needs-human`, `area: infra`

Create a Windows 11 Dev Drive (ReFS developer volume), exclude it from antivirus real-time scanning,
install Rust with rustup (MSVC toolchain) and the Visual Studio Build Tools it needs, then move the
repository onto the Dev Drive. Both steps need administrator rights, so the owner approves each one.

**Acceptance:** `cargo --version` works; the repository lives on the Dev Drive; a short note in
`STATUS.md` says where it is.

---

## M0-03 Rust workspace skeleton
**Labels:** `agent-ready`, `area: infra`

Create the Cargo workspace with: `rust-toolchain.toml` (pinned stable, with rustfmt and clippy),
workspace lints (`unsafe_code = "forbid"`, `missing_docs = "deny"`, clippy warnings as errors),
`rustfmt.toml`, `deny.toml` for `cargo-deny` allowing only permissive licences (MIT, Apache-2.0,
BSD, ISC, Zlib, Unicode), and one placeholder library crate that builds and has one test.

**Acceptance:** `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` and
`cargo deny check` all pass locally. Decision note for anything non-obvious.

---

## M0-04 Automated checks on Linux, Windows and macOS
**Labels:** `agent-ready`, `area: infra` · *after M0-03*

GitHub Actions workflow running format, clippy, tests and `cargo-deny` on all three systems for every
pull request. Then add these checks as **required** in the `main` ruleset.

**Acceptance:** a pull request with a formatting error fails; a clean one passes on all three.

---

## M0-05 Claude Code hooks
**Labels:** `agent-ready`, `area: infra` · *after M0-03*

Add Claude Code hooks (architecture section 18): format and fast lint after each edit, relevant
tests before finishing, and a block on risky commands. Check the current Claude Code hook format in
its documentation; don't guess it.

Include a second lock on owner approval: a hook that refuses any command that would add or remove
the `owner-approved` label on a pull request or issue, however it's written (`gh pr edit`,
`gh issue edit`, `gh api`, or anything else). Creating the label itself (M0-01) stays allowed.

**Acceptance:** demonstrated in the pull request: an edit gets formatted automatically; a force-push
is blocked; an attempt to add `owner-approved` to a pull request is blocked.

---

## M0-06 Check that flags weakened tests
**Labels:** `agent-ready`, `area: infra` · *after M0-04*

A CI check that flags any pull request that deletes tests, adds `#[ignore]`, or loosens a tolerance
constant, and posts a clear comment saying what it found. Such a pull request then also needs the
`owner-approved` label (M0-01) before it can merge.

**Acceptance:** a test pull request that deletes a test is flagged; a normal one isn't.

---

## M0-07 Rhino licence for automated use
**Labels:** `needs-human`, `area: kernel`

Find out whether Rhino's licence allows scripted, unattended use (driving Rhino through Rhino MCP to
create reference results). Read the licence and McNeel's published guidance, then write up for the
owner in plain language: what's allowed, what isn't, and the options. Architecture open decision 7.

**Acceptance:** a short answer the owner can decide on, posted in the issue.

---

## M0-08 Kernel evaluation: the 25 test cases
**Labels:** `blocked`, `area: kernel` · *blocked until the owner approves the spec*

Write the test operations of `docs/specs/m0-kernel-evaluation.md` section 3 as neutral case files
(inputs, options, what to measure), and a small harness outside the product code that can run a
case on any kernel and record outcome, measurements, time and an image.

---

## M0-09 Run the cases on solidcore
**Labels:** `blocked`, `area: kernel` · *after M0-08*

## M0-10 Run the cases on arris
**Labels:** `blocked`, `area: kernel` · *after M0-08*

## M0-11 Run the cases on monstertruck and truck
**Labels:** `blocked`, `area: kernel` · *after M0-08*

## M0-12 Run the curve and surface cases on curvo
**Labels:** `blocked`, `area: kernel` · *after M0-08*

For each: record every case as works / wrong / refuses / crashes, with measurements, time and an
image. Report crashes and wrong results upstream with a reproducible case, as the architecture asks.

---

## M0-13 Rhino reference results for the 25 cases
**Labels:** `blocked`, `area: kernel` · *after M0-07 and M0-08*

Automated through Rhino MCP if M0-07 allows it; otherwise a step-by-step script for the owner to
build the references by hand once.

---

## M0-14 Comparison gallery
**Labels:** `blocked`, `area: kernel` · *after M0-09 to M0-13*

One page: every case, every kernel, Rhino alongside, failures highlighted. The owner should be able
to review it in a few minutes.

---

## M0-15 Kernel recommendation and decision 0006
**Labels:** `blocked`, `area: kernel` · *after M0-14*

A plain-language report: which kernel for which operations, the gaps, what closing them would take.
Ends with a draft `docs/decisions/0006-kernel-choice.md` and a `needs-human` decision for the owner.

---

## M0-16 Hands-on test of SolidCraft: build on it, or start fresh?
**Labels:** `agent-ready`, `area: kernel`

Architecture open decision 1. Build and try SolidCraft; look at its size, structure, test quality and
how much of it matches Curva's rules (section 17). Write a decision note with options and a
recommendation, framed for the owner in terms of speed, risk and quality.

---

## M0-17 Licence and health check of supporting libraries
**Labels:** `agent-ready`, `area: docs`

For curvo, monstertruck, boolmesh, csgrs and openbim-ifc: licence, last release, activity, and fit.
Architecture open decisions 4 and 5. Note the result in a decision note; flag anything that isn't a
permissive licence.
