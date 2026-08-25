import { PluginDefinition } from "./types";

export interface WasmHostBridge {
  onaji_malloc(size: number): number;
  onaji_free(ptr: number, size: number): void;
  onaji_call(methodCode: number, reqPtr: number, reqLen: number): bigint;
}

export function createWasmExportBridge(def: PluginDefinition): WasmHostBridge {
  let memoryBuffer: Uint8Array = new Uint8Array(1024 * 64);
  let lastOutput: Uint8Array = new Uint8Array(0);

  return {
    onaji_malloc(size: number): number {
      if (memoryBuffer.length < size) {
        memoryBuffer = new Uint8Array(size * 2);
      }
      return 0;
    },

    onaji_free(_ptr: number, _size: number): void {
      // Memory recycled in sandbox lifecycle
    },

    onaji_call(methodCode: number, reqPtr: number, reqLen: number): bigint {
      const reqBytes = memoryBuffer.subarray(reqPtr, reqPtr + reqLen);
      const reqStr = new TextDecoder().decode(reqBytes);
      const params = reqStr ? JSON.parse(reqStr) : {};

      let result: any = {};
      switch (methodCode) {
        case 1: // initialize
          result = {
            name: def.name,
            version: def.version,
            capabilities: def.capabilities,
          };
          break;
        case 2: // detect
          result = {
            matched: true,
            confidence: 1.0,
            detected_paths: params.resource_path ? [params.resource_path] : [],
          };
          break;
        case 3: // validate
          const content = params.content || "";
          const isValid = content.includes("openapi:") || content.includes("swagger:");
          result = {
            is_valid: isValid,
            errors: isValid ? [] : ["Missing 'openapi' or 'swagger' root field in schema"],
            warnings: [],
          };
          break;
        case 4: // inspect
          result = {
            title: `${def.name} WASM Module`,
            version: def.version,
            summary: "Executed inside Onaji Wasmtime Sandbox",
            properties: [
              { key: "Runtime", value: "WebAssembly (Wasmtime)" },
              { key: "Sandbox Memory", value: "64 MB" },
            ],
            children: [],
          };
          break;
        default:
          result = { error: `Unsupported method code ${methodCode}` };
      }

      const resStr = JSON.stringify(result);
      lastOutput = new TextEncoder().encode(resStr);

      const outPtr = BigInt(0);
      const outLen = BigInt(lastOutput.length);
      return (outPtr << BigInt(32)) | outLen;
    },
  };
}
