import * as readline from "readline";
import { createPluginContext } from "./context";
import { PluginDefinition, RpcRequest, RpcResponse } from "./types";

export interface PluginRunner {
  handleMessage(line: string): Promise<string>;
  start(): void;
  definition: PluginDefinition;
}

export function definePlugin(definition: PluginDefinition): PluginRunner {
  const runner: PluginRunner = {
    definition,

    async handleMessage(line: string): Promise<string> {
      const trimmed = line.trim();
      if (!trimmed) return "";

      let req: RpcRequest;
      try {
        req = JSON.parse(trimmed);
      } catch (err: any) {
        const errRes: RpcResponse = {
          jsonrpc: "2.0",
          id: null,
          error: { code: -32700, message: `Parse error: ${err.message}` },
        };
        return JSON.stringify(errRes);
      }

      const response = await dispatchRpc(definition, req);
      return JSON.stringify(response);
    },

    start() {
      const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
        terminal: false,
      });

      rl.on("line", async (line) => {
        const out = await runner.handleMessage(line);
        if (out) {
          process.stdout.write(out + "\n");
        }
      });
    },
  };

  // Auto-start when executed directly from CLI / Node
  if (require.main && (require.main === module || require.main.children.includes(module))) {
    runner.start();
  }

  return runner;
}

export async function dispatchRpc(def: PluginDefinition, req: RpcRequest): Promise<RpcResponse> {
  const { id, method, params } = req;

  try {
    switch (method) {
      case "initialize": {
        return {
          jsonrpc: "2.0",
          id,
          result: {
            name: def.name,
            version: def.version,
            capabilities: def.capabilities,
          },
        };
      }

      case "plugin/detect": {
        if (!def.detect) {
          return {
            jsonrpc: "2.0",
            id,
            error: { code: -32601, message: "Method 'plugin/detect' not implemented by plugin" },
          };
        }
        const ctx = createPluginContext(params?.workspace_root || ".", params?.resource_path);
        const result = await def.detect(ctx);
        return { jsonrpc: "2.0", id, result };
      }

      case "plugin/validate": {
        if (!def.validate) {
          return {
            jsonrpc: "2.0",
            id,
            error: { code: -32601, message: "Method 'plugin/validate' not implemented by plugin" },
          };
        }
        const ctx = createPluginContext(params?.workspace_root || ".", params?.resource_path);
        const result = await def.validate(ctx);
        return { jsonrpc: "2.0", id, result };
      }

      case "plugin/inspect": {
        if (!def.inspect) {
          return {
            jsonrpc: "2.0",
            id,
            error: { code: -32601, message: "Method 'plugin/inspect' not implemented by plugin" },
          };
        }
        const ctx = createPluginContext(params?.workspace_root || ".", params?.resource_path);
        const result = await def.inspect(ctx);
        return { jsonrpc: "2.0", id, result };
      }

      case "plugin/execute": {
        if (!def.execute) {
          return {
            jsonrpc: "2.0",
            id,
            error: { code: -32601, message: "Method 'plugin/execute' not implemented by plugin" },
          };
        }
        const ctx = createPluginContext(
          params?.context?.workspace_root || ".",
          params?.context?.resource_path
        );
        const result = await def.execute(ctx, params?.target || "");
        return { jsonrpc: "2.0", id, result };
      }

      default:
        return {
          jsonrpc: "2.0",
          id,
          error: { code: -32601, message: `Method '${method}' not supported` },
        };
    }
  } catch (err: any) {
    return {
      jsonrpc: "2.0",
      id,
      error: { code: -32603, message: `Internal error in plugin: ${err.message}` },
    };
  }
}
