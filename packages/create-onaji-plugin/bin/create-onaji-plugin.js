#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const targetArg = process.argv[2];
if (!targetArg || targetArg === "--help" || targetArg === "-h") {
  console.log("Usage: npm create onaji-plugin <plugin-name>");
  process.exit(targetArg ? 0 : 1);
}

const pluginName = targetArg;
const targetDir = path.resolve(process.cwd(), pluginName);

if (fs.existsSync(targetDir)) {
  console.error(`Error: Directory '${pluginName}' already exists.`);
  process.exit(1);
}

console.log(`\n>>> Scaffolding Onaji plugin in ${targetDir}...\n`);

fs.mkdirSync(path.join(targetDir, "src"), { recursive: true });
fs.mkdirSync(path.join(targetDir, "test"), { recursive: true });

// 1. package.json
const pkgJson = {
  name: pluginName,
  version: "1.0.0",
  description: `Onaji automation plugin for ${pluginName}`,
  main: "dist/index.js",
  scripts: {
    build: "tsc",
    test: "node --test dist/**/*.test.js",
    start: "node dist/index.js",
  },
  dependencies: {
    "@onaji/plugin-sdk": "^0.1.0",
  },
  devDependencies: {
    "@types/node": "^20.11.0",
    typescript: "^5.3.3",
  },
};
fs.writeFileSync(path.join(targetDir, "package.json"), JSON.stringify(pkgJson, null, 2));

// 2. tsconfig.json
const tsconfig = {
  compilerOptions: {
    target: "ES2022",
    module: "commonjs",
    outDir: "./dist",
    rootDir: "./src",
    strict: true,
    esModuleInterop: true,
    skipLibCheck: true,
  },
  include: ["src/**/*"],
};
fs.writeFileSync(path.join(targetDir, "tsconfig.json"), JSON.stringify(tsconfig, null, 2));

// 3. onaji-plugin.yaml
const onajiPluginYaml = `name: "${pluginName}"
version: "1.0.0"
protocol: "1"

capabilities:
  - Detect
  - Inspect
  - Validate

runtime:
  command: "node"
  args:
    - "dist/index.js"

permissions:
  workspace_filesystem: true
  network: false
`;
fs.writeFileSync(path.join(targetDir, "onaji-plugin.yaml"), onajiPluginYaml);

// 4. src/index.ts
const srcIndex = `import { definePlugin } from "@onaji/plugin-sdk";

export default definePlugin({
  name: "${pluginName}",
  version: "1.0.0",

  capabilities: ["Detect", "Inspect", "Validate"],

  async detect(ctx) {
    const matched = await ctx.fs.exists("example.config");
    return {
      matched,
      confidence: matched ? 1.0 : 0.0,
      detected_paths: matched ? [ctx.fs.resolve("example.config")] : [],
    };
  },

  async validate(ctx) {
    const exists = await ctx.fs.exists("example.config");
    return {
      is_valid: exists,
      errors: exists ? [] : ["'example.config' was not found in workspace"],
      warnings: [],
    };
  },

  async inspect(ctx) {
    return {
      title: "${pluginName} Inspection Report",
      version: "1.0.0",
      summary: "Custom automation adapter powered by @onaji/plugin-sdk",
      properties: [
        { key: "Status", value: "Active" },
        { key: "Runtime", value: "Node.js" },
      ],
      children: [],
    };
  },
});
`;
fs.writeFileSync(path.join(targetDir, "src", "index.ts"), srcIndex);

// 5. README.md
const readme = `# ${pluginName}

Custom Onaji automation plugin built with \`@onaji/plugin-sdk\`.

## Build

\`\`\`bash
npm install
npm run build
\`\`\`

## Run with Onaji

Declare in your \`onaji.yaml\`:

\`\`\`yaml
plugins:
  ${pluginName}:
    command: "node"
    args:
      - "${pluginName}/dist/index.js"
\`\`\`
`;
fs.writeFileSync(path.join(targetDir, "README.md"), readme);

console.log(`✓ Plugin '${pluginName}' scaffolded successfully!`);
console.log(`\nNext steps:\n`);
console.log(`  cd ${pluginName}`);
console.log(`  npm install`);
console.log(`  npm run build\n`);
