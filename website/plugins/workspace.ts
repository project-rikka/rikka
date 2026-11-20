import { readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import type { Plugin } from "vite";

const SCHEME = "workspace:";

export function workspaceText(root: string): Plugin {
  return {
    name: "rikka-workspace-text",
    enforce: "pre",
    resolveId(id) {
      return id.startsWith(SCHEME) ? `\0${id}` : undefined;
    },
    load(id) {
      if (!id.startsWith(`\0${SCHEME}`)) {
        return undefined;
      }
      const name = id.slice(SCHEME.length + 1);
      const path = resolve(root, name);
      const inside = relative(root, path);
      if (inside.startsWith("..") || inside === "") {
        throw new Error(`${SCHEME}${name} is outside the workspace`);
      }
      this.addWatchFile(path);
      return `export default ${JSON.stringify(readFileSync(path, "utf8"))};\n`;
    },
  };
}
