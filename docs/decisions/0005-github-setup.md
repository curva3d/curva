# 0005 — How the GitHub project is set up

**Status:** Accepted · 8 November 2026

**Decision:**
- The project lives in its own **GitHub organisation**, not the owner's personal account, so the
  name is reserved early and others can join later.
- The repository is **public from the start.** Curva is open source anyway, and public
  repositories get free, unlimited automated testing on Linux, Windows and macOS. Private ones
  get a small monthly allowance, and macOS time counts ten times over.
- `main` is **protected:** changes arrive only through pull requests that pass all checks.
- **Agents work through the owner's own GitHub account.** Commits and pull requests appear under
  his name, with a "Co-authored-by: Claude" line so the history shows who wrote what.
- **The owner approves by label, not by button.** GitHub doesn't let anyone approve their own pull
  request, and agents act as the owner. So changes or deletions of *existing* tests, reference
  files and goldens, and any change to specs, can't merge until the owner adds the label `owner-approved`. An automated check enforces it, and
  the agents' permission rules block them from adding that label.
- Work is tracked in **GitHub Issues** with the labels and milestones listed in `setup/labels.md`.

**Rejected:** A separate GitHub account for the agents. It would allow GitHub's normal approval
button, but the owner wants everything under his own account. Trade-off accepted: the label rule is
enforced by the agents' permission rules and an automated check, not by GitHub's account system.
Adding new tests never needs approval, so the owner is asked rarely.

**Rejected:** A private repository until launch. It would ration automated testing on macOS, and
there is nothing to hide: the plan itself says Curva is open.

**Revisit if:** the owner wants to keep work private for a period. The switch is one setting.
