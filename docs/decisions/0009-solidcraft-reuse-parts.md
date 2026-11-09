# 0009: SolidCraft: build on it, or start fresh?

**Status:** parked, undecided. Reusing code is a later step (see 0010). This does not affect the choice of
engine. The test was cut short, as listed below.

**The question:** should Curva take SolidCraft as its starting point, or start fresh and reuse parts of it?

## What we found (November 2026)

- It builds on Windows first time, with no warnings, in about 21 minutes. The programs are large: about
  245 MB for the app and 175 MB for the command-line tool.
- The desktop app started with its sample part and stayed up for the 15 seconds watched. Its tools were not
  tried by hand.
- The command-line tool made a sample bracket in under a second, and the picture looks right.
- The bracket's STEP file read back as the same solid: same face, edge and corner counts, with volume and
  area matching to about 12 significant digits. This is SolidCraft checking its own file.
- It is about 1.1 million lines, started in October 2026, against Curva's aim of less code that a person
  can read. 90 source files are over 2,000 lines.
- It does not meet several Curva rules: 75 places in the main program, plus one device driver part, are
  exceptions to "no unsafe code", and public items need not be documented.
- The licence (MIT or Apache-2.0) is compatible with Curva.
- The STEP reader and writer (9,000 lines) and the sketch solver (33,000 lines) are separate parts.

## What was not tested

- SolidCraft's own tests (about 13,900): the run was stopped before any had run.
- Opening its STEP file in Rhino: the import timed out, so no independent check exists.
- Harder shapes: one simple bracket says little about difficult surface work.

## Options

1. **Start fresh and reuse the STEP and sketch parts (recommended).** Curva stays small and readable, and
   saves the work of writing those two parts. It is slower at first than taking everything.
2. **Build on all of SolidCraft.** The fastest way to a working program, but Curva inherits a million lines
   that nobody can read through and would have to be reworked to meet its own rules.
3. **Start fresh and reuse nothing.** The cleanest result, but the slowest, and repeats work that looks
   reusable.

## Before reusing either part

Run that part's own tests and check its results independently. This test did not establish that they are
reliable.
