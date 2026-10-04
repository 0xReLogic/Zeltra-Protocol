# Auditable Continuous Group Key Agreement

**Authors:** Easwar Vivek Mangipudi, Maddie Gorman, Sasha Levinshteyn  
**Published:** 2026-08-17  
**Category:** Cryptographic protocols  

## Summary
Continuous group key agreement (CGKA), the cryptographic core of Messaging Layer Security (MLS, RFC 9420), provides key management for large end-to-end encrypted group chats. It refreshes the group's keys as members join and leave, but offers no way for a designated auditor to recover past epoch keys, and no way to check that such recovery remains possible. Regulated deployments in finance, healthcare, and government therefore resort to plaintext server logging, abandoning end-to-end...
