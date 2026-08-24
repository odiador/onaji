#!/usr/bin/env node
const path = require("path");
const { definePlugin } = require("../../../packages/plugin-sdk/dist/index");

module.exports = definePlugin({
  name: "node-openapi",
  version: "1.0.0",

  capabilities: ["Detect", "Inspect", "Validate"],

  async detect(ctx) {
    const specPath = ctx.resourcePath || ctx.fs.resolve("backend/api/openapi.yaml");
    const matched = await ctx.fs.exists(specPath);
    return {
      matched,
      confidence: matched ? 1.0 : 0.0,
      detected_paths: matched ? [specPath] : [],
    };
  },

  async validate(ctx) {
    const specPath = ctx.resourcePath || ctx.fs.resolve("backend/api/openapi.yaml");
    const exists = await ctx.fs.exists(specPath);
    if (!exists) {
      return {
        is_valid: false,
        errors: [`File not found: ${specPath}`],
        warnings: [],
      };
    }

    const content = await ctx.fs.readFile(specPath);
    const isValid = content.includes("openapi:") || content.includes("swagger:");
    return {
      is_valid: isValid,
      errors: isValid ? [] : ["Missing 'openapi' or 'swagger' root field in schema"],
      warnings: [],
    };
  },

  async inspect(ctx) {
    const specPath = ctx.resourcePath || ctx.fs.resolve("backend/api/openapi.yaml");
    const exists = await ctx.fs.exists(specPath);
    const content = exists ? await ctx.fs.readFile(specPath) : "";

    const titleMatch = content.match(/title:\s*["']?([^"'\n]+)/);
    const versionMatch = content.match(/version:\s*["']?([^"'\n]+)/);
    const endpointMatches = content.match(/^\s{2}\/[a-zA-Z0-9_\-\/]+:/gm) || [];

    return {
      title: "OpenAPI Contract (@onaji/plugin-sdk)",
      version: versionMatch ? versionMatch[1].trim() : "1.0.0",
      summary: `Extracted via @onaji/plugin-sdk runtime (${endpointMatches.length} endpoints)`,
      properties: [
        { key: "API Title", value: titleMatch ? titleMatch[1].trim() : "Unknown" },
        { key: "Total Endpoints", value: endpointMatches.length.toString() },
        { key: "SDK Engine", value: "@onaji/plugin-sdk v0.1.0" },
      ],
      children: [],
    };
  },
});
