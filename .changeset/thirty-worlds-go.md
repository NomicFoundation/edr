---
"@nomicfoundation/edr": patch
---

Fixed detection of function signature for calls to proxies.

Previously, a call to a proxy contract was attributed to the proxy instead of the implementation. Now, it is correctly attributed to the implementation, allowing detection of the correct function signature.
