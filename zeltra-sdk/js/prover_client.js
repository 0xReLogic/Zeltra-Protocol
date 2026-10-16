/**
 * Zeltra Protocol — Client WebAssembly Groth16 Prover Orchestrator (DEC-041)
 * JavaScript (ESM) implementation.
 */

export class ZeltraBrowserProver {
    /**
     * Creates a new ZeltraBrowserProver instance.
     * @param {string} workerScriptUrl URL or path to `zeltra_prover_worker.js`
     * @param {string} [defaultWasmPath] Optional custom URL or path to `zeltra_sdk.js` WASM loader
     */
    constructor(workerScriptUrl, defaultWasmPath) {
        this.workerScriptUrl = workerScriptUrl;
        this.defaultWasmPath = defaultWasmPath;
    }

    /**
     * Executes Groth16 proving for 1-in-1-out PrivateNoteCircuit (16 Public Inputs, DEC-035A/B).
     * @param {object} witness
     * @param {Uint8Array|ArrayBuffer} pkBuffer
     * @param {object} [options]
     * @returns {Promise<object>}
     */
    async provePrivateNote(witness, pkBuffer, options) {
        return this.executeWorkerProof('PROVE_PRIVATE_NOTE', witness, pkBuffer, options);
    }

    /**
     * Executes Groth16 proving for Universal 2-in-2-out JoinSplitCircuit (19 Public Inputs, DEC-036A).
     * @param {object} witness
     * @param {Uint8Array|ArrayBuffer} pkBuffer
     * @param {object} [options]
     * @returns {Promise<object>}
     */
    async proveJoinSplit(witness, pkBuffer, options) {
        return this.executeWorkerProof('PROVE_JOINSPLIT', witness, pkBuffer, options);
    }

    /**
     * Executes instant Groth16 proving for CEX SelectiveDisclosureCircuit (5 Public Inputs, DEC-040A).
     * @param {object} witness
     * @param {Uint8Array|ArrayBuffer} pkBuffer
     * @param {object} [options]
     * @returns {Promise<object>}
     */
    async proveSelectiveDisclosure(witness, pkBuffer, options) {
        return this.executeWorkerProof('PROVE_DISCLOSURE', witness, pkBuffer, options);
    }

    /**
     * Internal Disposable Worker runner.
     * Guarantees worker.terminate() is called in all execution branches.
     * @private
     */
    async executeWorkerProof(action, witness, pkBuffer, options) {
        const timeoutMs = (options && options.timeoutMs) ? options.timeoutMs : 30000;
        const wasmPath = (options && options.wasmPath) ? options.wasmPath : this.defaultWasmPath;

        const worker = new Worker(this.workerScriptUrl, { type: 'module' });

        try {
            return await new Promise((resolve, reject) => {
                const timer = setTimeout(() => {
                    reject(new Error(`Prover execution timed out after ${timeoutMs}ms (Action: ${action})`));
                }, timeoutMs);

                worker.onmessage = (event) => {
                    const data = event.data || {};

                    if (data.status === 'PROGRESS') {
                        if (options && typeof options.onProgress === 'function') {
                            options.onProgress({
                                stage: data.stage || 'UNKNOWN',
                                percentage: data.percentage || 0,
                            });
                        }
                    } else if (data.status === 'SUCCESS') {
                        clearTimeout(timer);
                        resolve(data.result);
                    } else if (data.status === 'ERROR') {
                        clearTimeout(timer);
                        const err = new Error(data.error || 'Unknown Groth16 proving error in worker');
                        if (data.details) {
                            Object.assign(err, data.details);
                        }
                        reject(err);
                    }
                };

                worker.onerror = (err) => {
                    clearTimeout(timer);
                    reject(err instanceof Error ? err : new Error(`Web Worker execution error: ${String(err)}`));
                };

                // Prepare transferable ArrayBuffer for zero-copy IPC
                let transferBuffer;
                if (pkBuffer instanceof ArrayBuffer) {
                    transferBuffer = pkBuffer.slice(0);
                } else if (ArrayBuffer.isView(pkBuffer)) {
                    const uint8 = new Uint8Array(pkBuffer.buffer, pkBuffer.byteOffset, pkBuffer.byteLength);
                    transferBuffer = uint8.slice().buffer;
                } else {
                    reject(new Error('pkBuffer must be Uint8Array or ArrayBuffer'));
                    return;
                }

                worker.postMessage(
                    {
                        action,
                        witness,
                        provingKey: transferBuffer,
                        wasmPath,
                    },
                    [transferBuffer]
                );
            });
        } finally {
            // DEC-041 INVARIANT 2: Mandatory Disposable Worker Lifecycle
            worker.terminate();
        }
    }
}
