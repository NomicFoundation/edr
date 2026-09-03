---
"@nomicfoundation/edr": patch
---

Fixed a leak where a subscription callback that referenced its provider kept the provider, and the OS thread it owns, alive after it was no longer referenced.
