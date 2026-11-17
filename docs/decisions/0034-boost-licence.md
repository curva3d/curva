# 0034: The Boost licence is allowed for all of Curva

**Status:** Decided by the owner in chat · 17 November 2026.

**What was decided.** Outside libraries under the Boost Software Licence 1.0 (BSL-1.0) may be used
anywhere in Curva, like those under MIT or Apache-2.0. It is on the allowed list in `deny.toml`
and `about.toml`.

**Why.** It is as permissive as MIT: anyone may use, change and sell the code; the only condition
is keeping its licence text with copies of the source, not in the finished program. It works with
both of Curva's licences (MIT or Apache-2.0 for the libraries, GPL for the application, decision
0025) and is approved by the OSI and the Free Software Foundation. The first library that needs it
is clipper2-rust, which manifold-rust (mesh booleans, decision 0032) uses.

**Rejected.** Allowing it for clipper2-rust only, as was done for acadrust's MPL-2.0: the owner
preferred not to be asked again for a licence this loose. Refusing it, which would have ruled out
manifold-rust.

**Revisit if** a library under it turns out to carry other conditions in its own files.
