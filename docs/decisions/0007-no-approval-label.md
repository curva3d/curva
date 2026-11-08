# 0007 — Agents merge their own work; no approval label

**Status:** Accepted · 8 November 2026 · Amends decision 0005 and architecture sections 15 to 19

**Decision:** The owner no longer approves or merges pull requests. The `owner-approved` label and
the check that would have enforced it are dropped. An agent merges its own pull request once the
automated checks pass on Linux, Windows and macOS. Those checks stay mandatory on `main`.

**Why:** Clicking and labelling for every change took the owner's time without giving him anything
he could judge, since he doesn't read code.

**What stays the owner's:** specs, product and interface behaviour, releases and anything that would
weaken a test. He decides these in conversation, not through a GitHub control.

**The risk we accept:** nothing but the agents' own rules now stops an agent from changing an
existing test or reference file so that its work passes. GitHub would not block it. A planned check
(M0-06) will flag deleted tests, ignored tests and loosened tolerances, so the agent has to stop and
ask him.

**Revisit if:** a wrong result turns out to have been hidden by a changed test, or more people join.
