import { realpathSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "astro/config";
import { rikkaTheme } from "./plugins/theme";
import { workspaceText } from "./plugins/workspace";
import { site } from "./src/site";

const root = fileURLToPath(new URL(".", import.meta.url));
const workspace = join(root, "..");
const modules = realpathSync(join(root, "node_modules"));

export default defineConfig({
  site: site.url,
  cacheDir: "./.astro/cache",
  compressHTML: true,
  devToolbar: { enabled: false },
  security: {
    csp: {
      directives: [
        "default-src 'self'",
        "img-src 'self' data:",
        "font-src 'self'",
        "connect-src 'self'",
        "object-src 'none'",
        "base-uri 'none'",
        "form-action 'none'",
      ],
      styleDirective: {
        resources: [
          { resource: "'self'", kind: "element" },
          { resource: "'unsafe-inline'", kind: "attribute" },
        ],
      },
    },
  },
  markdown: { syntaxHighlight: false },
  vite: {
    cacheDir: "./.astro/vite",
    plugins: [
      workspaceText(workspace),
      rikkaTheme({
        directory: join(workspace, "crates/graphics/rikka-theme/themes"),
        light: "rikka-light",
        dark: "rikka",
      }),
    ],
    server: { fs: { allow: [root, modules] } },
  },
});
