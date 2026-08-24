import * as fs from "fs";
import * as path from "path";
import { Logger, PluginContext, WorkspaceFileSystem } from "./types";

export function createPluginContext(workspaceRoot: string, resourcePath?: string): PluginContext {
  const resolvedRoot = path.resolve(workspaceRoot);

  const fileSystem: WorkspaceFileSystem = {
    async exists(relPath: string): Promise<boolean> {
      const full = path.resolve(resolvedRoot, relPath);
      return fs.existsSync(full);
    },

    async readFile(relPath: string, encoding: BufferEncoding = "utf-8"): Promise<string> {
      const full = path.resolve(resolvedRoot, relPath);
      return fs.promises.readFile(full, encoding);
    },

    async writeFile(relPath: string, content: string): Promise<void> {
      const full = path.resolve(resolvedRoot, relPath);
      await fs.promises.mkdir(path.dirname(full), { recursive: true });
      return fs.promises.writeFile(full, content, "utf-8");
    },

    async listDir(relPath: string): Promise<string[]> {
      const full = path.resolve(resolvedRoot, relPath);
      if (!fs.existsSync(full)) return [];
      return fs.promises.readdir(full);
    },

    resolve(relPath: string): string {
      return path.resolve(resolvedRoot, relPath);
    },
  };

  const logger: Logger = {
    info(msg: string, ...args: any[]) {
      console.error(`[INFO] [${msg}]`, ...args);
    },
    warn(msg: string, ...args: any[]) {
      console.error(`[WARN] [${msg}]`, ...args);
    },
    error(msg: string, ...args: any[]) {
      console.error(`[ERROR] [${msg}]`, ...args);
    },
    debug(msg: string, ...args: any[]) {
      console.error(`[DEBUG] [${msg}]`, ...args);
    },
  };

  return {
    workspaceRoot: resolvedRoot,
    resourcePath: resourcePath ? path.resolve(resolvedRoot, resourcePath) : undefined,
    fs: fileSystem,
    logger,
  };
}
