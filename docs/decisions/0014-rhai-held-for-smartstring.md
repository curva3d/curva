# 0014: Rhai scripting is held until smartstring is gone

**Status:** Decided by the owner in chat · 9 November 2026

**Decision:** Rhai scripting does not ship while Rhai still depends on `smartstring`, which is MPL-2.0 and unmaintained (RUSTSEC-2026-0249). The finished work waits on the branch `m1-rhai-scripting`.

**What was checked:** Rhai 1.27.0 is the latest release. It offers `compact_str` as an optional feature, but `smartstring` stays a required dependency even with that feature on, so it remains in the dependency tree.

**Approved for when scripting ships:** `CC0-1.0`, for `tiny-keccak`, which is public domain and build-time only.

**To release it:** when a Rhai release no longer requires `smartstring`, update Rhai on that branch, confirm `smartstring` is gone from the tree, and merge once all checks pass.
