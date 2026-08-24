#!/usr/bin/env node
const fs = require("fs");
const path = require("path");
const readline = require("readline");

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
});

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;

  try {
    const req = JSON.parse(trimmed);
    const res = handleRpc(req);
    process.stdout.write(JSON.stringify(res) + "\n");
  } catch (err) {
    const errRes = {
      jsonrpc: "2.0",
      id: null,
      error: { code: -32700, message: "Parse error: " + err.message },
    };
    process.stdout.write(JSON.stringify(errRes) + "\n");
  }
});

function handleRpc(req) {
  const { id, method, params } = req;

  switch (method) {
    case "initialize":
      return {
        jsonrpc: "2.0",
        id,
        result: {
          name: "node-openapi",
          version: "1.0.0",
          capabilities: ["Detect", "Inspect", "Validate"],
        },
      };

    case "plugin/detect": {
      const root = params.workspace_root || ".";
      const specPath = params.resource_path || path.join(root, "backend/api/openapi.yaml");
      const matched = fs.existsSync(specPath);
      return {
        jsonrpc: "2.0",
        id,
        result: {
          matched,
          confidence: matched ? 1.0 : 0.0,
          detected_paths: matched ? [specPath] : [],
        },
      };
    }

    case "plugin/validate": {
      const root = params.workspace_root || ".";
      const specPath = params.resource_path || path.join(root, "backend/api/openapi.yaml");
      if (!fs.existsSync(specPath)) {
        return {
          jsonrpc: "2.0",
          id,
          result: {
            is_valid: false,
            errors: [`File not found: ${specPath}`],
            warnings: [],
          },
        };
      }

      const content = fs.readFileSync(specPath, "utf-8");
      const isValid = content.includes("openapi:") || content.includes("swagger:");
      return {
        jsonrpc: "2.0",
        id,
        result: {
          is_valid: isValid,
          errors: isValid ? [] : ["Missing 'openapi' or 'swagger' root field"],
          warnings: [],
        },
      };
    }

    case "plugin/inspect": {
      const root = params.workspace_root || ".";
      const specPath = params.resource_path || path.join(root, "backend/api/openapi.yaml");
      const content = fs.existsSync(specPath) ? fs.readFileSync(specPath, "utf-8") : "";

      const titleMatch = content.match(/title:\s*["']?([^"'\n]+)/);
      const versionMatch = content.match(/version:\s*["']?([^"'\n]+)/);
      const endpointMatches = content.match(/^\s{2}\/[a-zA-Z0-9_\-\/]+:/gm) || [];

      return {
        jsonrpc: "2.0",
        id,
        result: {
          title: "OpenAPI Contract (Node.js Plugin)",
          version: versionMatch ? versionMatch[1].trim() : "1.0.0",
          summary: `Extracted via Node.js external process plugin (${endpointMatches.length} endpoints)`,
          properties: [
            { key: "API Title", value: titleMatch ? titleMatch[1].trim() : "Unknown" },
            { key: "Total Endpoints", value: endpointMatches.length.toString() },
            { key: "Plugin Engine", value: "Node.js v" + process.version },
          ],
          children: [],
        },
      };
    }

    default:
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32601, message: `Method '${method}' not supported by Node plugin` },
      };
  }
}
