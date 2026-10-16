/**
 * Zeltra Protocol — Client WebAssembly Groth16 Prover Orchestrator (DEC-041)
 *
 * Implements non-blocking, zero-OOM in-browser Groth16 proving:
 * - Spawns a dedicated Web Worker on demand per proof request
 * - Transfers proving key buffers zero-copy (Transferable Objects)
 * - Emits progress heartbeats without blocking the UI main thread
 * - Enforces the Disposable Worker Lifecycle: worker.terminate() inside finally
 * - Enforces Watchdog Timer (default 30s) against hanging proof computations
 */

export interface ProverResult {
    proofA: string;         // EVM G1 point (-A), 0x-prefixed 128 hex chars
    proofB: string;         // EVM G2 point (B), 0x-prefixed 256 hex chars
    proofC: string;         // EVM G1 point (C), 0x-prefixed 128 hex chars
    publicInputs: string[]; // Canonical 32-byte EVM big-endian hex elements (16, 19, or 5 PIs)
    provingTimeMs: number;  // Measured proof computation duration in milliseconds
}

export interface ProverProgress {
    stage: string;
    percentage: number;
}

export interface ProverOptions {
    timeoutMs?: number;
    onProgress?: (progress: ProverProgress) => void;
    wasmPath?: string;
}

export class ZeltraBrowserProver {
    private workerScriptUrl: string;
    private defaultWasmPath?: string;

    /**
     * Creates a new ZeltraBrowserProver instance.
     * @param workerScriptUrl URL or path to `zeltra_prover_worker.js`
     * @param defaultWasmPath Optional custom URL or path to `zeltra_sdk.js` WASM loader
     */
    constructor(workerScriptUrl: string, defaultWasmPath?: string) {
        this.workerScriptUrl = workerScriptUrl;
        this.defaultWasmPath = defaultWasmPath;
    }

    /**
     * Executes Groth16 proving for 1-in-1-out PrivateNoteCircuit (16 Public Inputs, DEC-035A/B).
     */
    public async provePrivateNote(
        witness: object,
        pkBuffer: Uint8Array | ArrayBuffer,
        options?: ProverOptions
    ): Promise<ProverResult> {
        return this.executeWorkerProof('PROVE_PRIVATE_NOTE', witness, pkBuffer, options);
    }

    /**
     * Executes Groth16 proving for Universal 2-in-2-out JoinSplitCircuit (19 Public Inputs, DEC-036A).
     */
    public async proveJoinSplit(
        witness: object,
        pkBuffer: Uint8Array | ArrayBuffer,
        options?: ProverOptions
    ): Promise<ProverResult> {
        return this.executeWorkerProof('PROVE_JOINSPLIT', witness, pkBuffer, options);
    }

    /**
     * Executes instant Groth16 proving for CEX SelectiveDisclosureCircuit (5 Public Inputs, DEC-040A).
     */
    public async proveSelectiveDisclosure(
        witness: object,
        pkBuffer: Uint8Array | ArrayBuffer,
        options?: ProverOptions
    ): Promise<ProverResult> {
        return this.executeWorkerProof('PROVE_DISCLOSURE', witness, pkBuffer, options);
    }

    /**
     * Internal Disposable Worker runner.
     * Guarantees worker.terminate() is called in all execution branches.
     */
    private async executeWorkerProof(
        action: string,
        witness: object,
        pkBuffer: Uint8Array | ArrayBuffer,
        options?: ProverOptions
    ): Promise<ProverResult> {
        const timeoutMs = options?.timeoutMs ?? 30000;
        const wasmPath = options?.wasmPath || this.defaultWasmPath;

        const worker = new Worker(this.workerScriptUrl, { type: 'module' });

        try {
            return await new Promise<ProverResult>((resolve, reject) => {
                const timer = setTimeout(() => {
                    reject(new Error(`Prover execution timed out after ${timeoutMs}ms (Action: ${action})`));
                }, timeoutMs);

                worker.onmessage = (event: MessageEvent) => {
                    const data = event.data || {};

                    if (data.status === 'PROGRESS') {
                        if (options?.onProgress) {
                            options.onProgress({
                                stage: data.stage || 'UNKNOWN',
                                percentage: data.percentage || 0,
                            });
                        }
                    } else if (data.status === 'SUCCESS') {
                        clearTimeout(timer);
                        resolve(data.result as ProverResult);
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
                let transferBuffer: ArrayBuffer;
                if (pkBuffer instanceof ArrayBuffer) {
                    transferBuffer = pkBuffer.slice(0);
                } else if (ArrayBuffer.isView(pkBuffer)) {
                    // Copy slice to preserve original buffer in main thread
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
            // Immediately terminate worker isolate to force OS memory deallocation
            worker.terminate();
        }
    }
}
