---
"@nomicfoundation/edr": patch
---

Made decoding solx DWARF debug info scale close to linearly with contract size. Loading build info for large solx-compiled projects previously grew super-linearly, because each instruction re-scanned the inlined-range, AST-span and function tables; those lookups are now a single sweep plus per-decode caches. Decoded stack traces are unchanged.
