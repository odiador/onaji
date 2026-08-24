import { dispatchRpc } from "./definePlugin";
import { PluginDefinition, RpcResponse } from "./types";

export interface TestPluginOptions {
  workspaceRoot?: string;
  expectedCapabilities?: string[];
}

export async function testPluginProtocol(def: PluginDefinition, options: TestPluginOptions = {}): Promise<void> {
  const root = options.workspaceRoot || ".";

  // 1. Test initialize
  const initRes = await dispatchRpc(def, {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocol_version: "1", workspace_root: root },
  });

  if (initRes.error) {
    throw new Error(`Plugin failed initialize: ${initRes.error.message}`);
  }

  if (!initRes.result || initRes.result.name !== def.name) {
    throw new Error(`Plugin name mismatch in initialize result: expected '${def.name}', got '${initRes.result?.name}'`);
  }

  // 2. Test unknown method handling
  const unknownRes = await dispatchRpc(def, {
    jsonrpc: "2.0",
    id: 2,
    method: "plugin/non_existent_method",
  });

  if (!unknownRes.error || unknownRes.error.code !== -32601) {
    throw new Error("Plugin should return code -32601 (Method not found) for unknown methods");
  }

  // 3. Test declared capabilities
  for (const cap of def.capabilities) {
    const method = `plugin/${cap.toLowerCase()}`;
    const capRes = await dispatchRpc(def, {
      jsonrpc: "2.0",
      id: 3,
      method,
      params: { workspace_root: root },
    });

    if (capRes.error && capRes.error.code === -32601) {
      throw new Error(`Plugin declared capability '${cap}' but did not implement '${method}'`);
    }
  }
}
