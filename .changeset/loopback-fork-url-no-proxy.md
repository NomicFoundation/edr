---
"@nomicfoundation/edr": patch
---

Fixed fork requests to loopback URLs (`localhost`, `*.localhost`, `127.0.0.0/8`, `0.0.0.0`, `::1`) being sent through `HTTP_PROXY`/`HTTPS_PROXY`.
