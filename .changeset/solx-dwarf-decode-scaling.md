---
"@nomicfoundation/edr": patch
---

Made loading build info for large solx-compiled projects faster: decoding solx DWARF debug info now scales close to linearly with contract size.
