/**
 * Ephemeral Web Worker Prover & Memory Guard for Zeltra Protocol (DEC-038C / INV-REL-4)
 *
 * Implements the Ephemeral Web Worker lifecycle for browser-based Groth16 spend proof generation:
 * 1. Spawns a dedicated Web Worker on demand only when generating spend proofs.
 * 2. Isolates WASM 32-bit linear memory pages inside the worker thread.
 * 3. Immediately terminates the worker (`worker.terminate()`) upon proof completion or failure,
 *    reclaiming 100% of linear memory back to the host browser.
 * 4. Zero ZK proving overhead during deposit inflow (direct Poseidon + AEAD memo < 100 KB).
 */

/**
 * Worker script code executed inside the Web Worker context.
 * In a bundled application, this can be loaded via a dedicated worker bundle URL or Blob URL.
 */
export const WORKER_INLINE_SCRIPT = `
self.onmessage = async function(event) {
    const { action, payload } = event.data;
    if (action === "GENERATE_SPEND_PROOF") {
        try {
            // Import WASM module in worker scope
            const wasm = await import("./zeltra_sdk.js");
            if (wasm.default) {
                await wasm.default();
            }
            
            const proof = wasm.generate_spend_proof(
                payload.noteHex,
                payload.recipientHex,
                payload.amountHex,
                payload.merklePathHex
            );
            
            self.postMessage({ success: true, proof });
        } catch (err) {
            self.postMessage({ success: false, error: err.toString() });
        }
    }
};
`;

/**
 * Executes a ZK Groth16 spend proof in an isolated ephemeral Web Worker.
 * Guarantees that worker.terminate() is called in all outcomes (success or error).
 *
 * @param {Object} spendPayload - The spend inputs required for Groth16 proving.
 * @param {string} [workerUrl] - Optional URL to the worker bundle. If omitted, uses an inline Blob URL.
 * @returns {Promise<Object>} Resolves with the generated proof.
 */
export async function proveSpendInEphemeralWorker(spendPayload, workerUrl) {
    let worker;
    let blobUrl;

    if (workerUrl) {
        worker = new Worker(workerUrl, { type: "module" });
    } else {
        const blob = new Blob([WORKER_INLINE_SCRIPT], { type: "application/javascript" });
        blobUrl = URL.createObjectURL(blob);
        worker = new Worker(blobUrl, { type: "module" });
    }

    try {
        const proof = await new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error("Ephemeral worker spend proving timed out after 30s"));
            }, 30000);

            worker.onmessage = (event) => {
                clearTimeout(timeout);
                if (event.data.success) {
                    resolve(event.data.proof);
                } else {
                    reject(new Error(event.data.error || "Unknown proving error in worker"));
                }
            };

            worker.onerror = (err) => {
                clearTimeout(timeout);
                reject(err);
            };

            worker.postMessage({
                action: "GENERATE_SPEND_PROOF",
                payload: spendPayload,
            });
        });

        return proof;
    } finally {
        // INV-REL-4: Terminate worker immediately to reclaim 100% linear memory
        worker.terminate();
        if (blobUrl) {
            URL.revokeObjectURL(blobUrl);
        }
    }
}
