# solidcore-ops (patched copy)

A copy of `solidcore-ops` from [solidcore](https://github.com/tangency-labs/solidcore) (Apache 2.0,
see `LICENSE` and `NOTICE`) at commit `9342f32`, with one change, kept in `curva.patch`:

- `session::with_own_kernel`: inside it, a thread's engine calls run on a kernel of its own instead
  of waiting for the one kernel the whole program shares. Curva uses it to build solids, cut plans
  and find hidden lines on every core at once. Files changed carry a "Changed by Curva" line.

Nothing else is changed. The change was offered to solidcore's authors; **when solidcore has it,
Curva goes back to the released engine and deletes this folder**, after its tests and timings pass
on it (decision 0037).
