# Draft for solidcore: an offset sheet has no record until written and read back

**Not posted.** Found on 14 November 2026 at the revision Curva pins, `9342f32`, while holding
bodies parsed (`docs/reviews/code-health-2026-11-14.md`, fix 1). Posting needs the owner's approval,
as the earlier reports did. Curva works around it by writing every new body to the native text and
reading it back once (`Body::made` in `crates/curva-kernel/src/body.rs`).

---

### What happens

A planar sheet offset with `offset_sheet` cannot be described: `to_record` fails with
`Unsupported("a face surface with no NURBS form has no exact record")`. The same body written with
`to_smb(true)` and read back with `from_smb` describes fine, because the reader turns the offset
surface into NURBS.

### Reproduction

1. A ruled sheet between the lines (0,0,0)–(10,0,0) and (0,10,0)–(10,10,0).
2. `offset_sheet(&body, 2.0)`.
3. `body.to_record()` returns the error above.
4. `Body::from_smb(body.to_smb(true)?, true)?.to_record()` succeeds.

### Expected

`to_record` gives the offset face's exact surface (or its NURBS form) without a round trip through
the native format.
