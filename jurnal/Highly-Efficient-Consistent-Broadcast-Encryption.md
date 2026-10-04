# Highly Efficient Consistent Broadcast Encryption

**Authors:** Konstantin Gegier, Eike Kiltz, Roman Langrehr, Guilherme Rito  
**Published:** 2026-07-13  
**Category:** Public-key cryptography  

## Summary
Public Key Encryption for Broadcast ($\mathsf{PKEBC}$) is a multi-recipient encryption primitive that guarantees decryption consistency across all designated recipients. Concretely, if a ciphertext $c$ is encrypted for Bob and Charlie, and Bob's decryption yields a message $m$, then Charlie's decryption of $c$ must also succeed and produce the same $m$. This property, though seemingly natural, is essential in secure group messaging, where consistent message delivery is often implicitly...
