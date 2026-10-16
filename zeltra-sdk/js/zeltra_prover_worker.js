/**
 * Zeltra Protocol — Dedicated In-Browser WebAssembly Groth16 Prover Worker (DEC-041)
 *
 * Implements the Disposable Web Worker execution runtime for:
 * 1. 1-in-1-out PrivateNoteCircuit (16 Public Inputs, DEC-035A/B)
 * 2. Universal 2-in-2-out JoinSplitCircuit (19 Public Inputs, DEC-036A)
 * 3. CEX SelectiveDisclosureCircuit (5 Public Inputs, DEC-040A)
 *
 * Architecture Mandates:
 * - Single-threaded pure WASM execution (Zero COOP/COEP header dependency)
 * - Zero-copy buffer transfer for proving keys
 * - Immediate termination pattern (Anti-OOM Lifecycle)
 */

let wasmInstance = null;

/**
 * Dynamically resolves and initializes the Zeltra SDK WebAssembly module.
 * @param {string} [customWasmPath]
 */
async function loadWasmModule(customWasmPath) {
    if (wasmInstance) {
        return wasmInstance;
    }

    const modulePath = customWasmPath || './zeltra_sdk.js';
    const wasm = await import(modulePath);

    if (typeof wasm.default === 'function') {
        await wasm.default();
    }

    if (typeof wasm.init_wasm_panic_hook === 'function') {
        wasm.init_wasm_panic_hook();
    }

    wasmInstance = wasm;
    return wasmInstance;
}

/**
 * Normalizes proving key input to Uint8Array.
 * @param {ArrayBuffer|Uint8Array} pk
 * @returns {Uint8Array}
 */
function toUint8Array(pk) {
    if (pk instanceof Uint8Array) {
        return pk;
    }
    if (pk instanceof ArrayBuffer) {
        return new Uint8Array(pk);
    }
    if (ArrayBuffer.isView(pk)) {
        return new Uint8Array(pk.buffer, pk.byteOffset, pk.byteLength);
    }
    throw new Error('Proving key must be an ArrayBuffer or Uint8Array');
}

/**
 * Worker message dispatcher.
 */
self.onmessage = async function (event) {
    const { action, witness, provingKey, wasmPath } = event.data || {};

    try {
        if (!action) {
            throw new Error('Missing "action" in worker message');
        }
        if (!witness) {
            throw new Error('Missing "witness" payload in worker message');
        }
        if (!provingKey) {
            throw new Error('Missing "provingKey" buffer in worker message');
        }

        const wasm = await loadWasmModule(wasmPath);
        const pkBytes = toUint8Array(provingKey);
        const witnessJson = typeof witness === 'string' ? witness : JSON.stringify(witness);

        self.postMessage({ status: 'PROGRESS', stage: 'WITNESS_EVALUATION', percentage: 20 });

        let result;
        switch (action) {
            case 'PROVE_PRIVATE_NOTE': {
                self.postMessage({ status: 'PROGRESS', stage: 'GROTH16_MSM_FFT', percentage: 50 });
                result = wasm.wasm_prove_private_note(witnessJson, pkBytes);
                break;
            }

            case 'PROVE_JOINSPLIT': {
                self.postMessage({ status: 'PROGRESS', stage: 'GROTH16_MSM_FFT', percentage: 50 });
                result = wasm.wasm_prove_joinsplit(witnessJson, pkBytes);
                break;
            }

            case 'PROVE_DISCLOSURE':
            case 'PROVE_SELECTIVE_DISCLOSURE': {
                self.postMessage({ status: 'PROGRESS', stage: 'GROTH16_FAST_DISCLOSURE', percentage: 60 });
                result = wasm.wasm_prove_selective_disclosure(witnessJson, pkBytes);
                break;
            }

            default:
                throw new Error(`Unsupported prover action: ${action}`);
        }

        self.postMessage({ status: 'PROGRESS', stage: 'COMPLETED', percentage: 100 });
        self.postMessage({ status: 'SUCCESS', result });
    } catch (err) {
        const errorMessage = (err && (err.message || err.errorType)) ? (err.message || err.errorType) : String(err);
        self.postMessage({
            status: 'ERROR',
            error: errorMessage,
            details: typeof err === 'object' ? err : { message: String(err) }
        });
    }
};
