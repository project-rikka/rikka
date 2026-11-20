import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Plugin } from "vite";
import { readPalette, ROLES, type Palette } from "./palette";

const MODULE = "virtual:rikka-theme";
const STYLESHEET = "virtual:rikka-theme.css";

export interface ThemeSources {
  readonly directory: string;
  readonly light: string;
  readonly dark: string;
}

export function rikkaTheme(sources: ThemeSources): Plugin {
  const paths = {
    light: join(sources.directory, `${sources.light}.toml`),
    dark: join(sources.directory, `${sources.dark}.toml`),
  };
  const load = () => ({
    light: readPalette(readFileSync(paths.light, "utf8"), sources.light),
    dark: readPalette(readFileSync(paths.dark, "utf8"), sources.dark),
  });
  return {
    name: "rikka-theme",
    resolveId(id) {
      return id === MODULE || id === STYLESHEET ? `\0${id}` : undefined;
    },
    load(id) {
      if (id !== `\0${MODULE}` && id !== `\0${STYLESHEET}`) {
        return undefined;
      }
      this.addWatchFile(paths.light);
      this.addWatchFile(paths.dark);
      const { light, dark } = load();
      if (light.variant !== "light" || dark.variant !== "dark") {
        throw new Error(
          `rikka-theme: ${light.slug} must be light and ${dark.slug} dark`,
        );
      }
      return id === `\0${MODULE}`
        ? `export const light = ${JSON.stringify(light)};\nexport const dark = ${JSON.stringify(dark)};\n`
        : stylesheet(light, dark);
    },
  };
}

function stylesheet(light: Palette, dark: Palette): string {
  const declarations = ROLES.map(
    (role) =>
      `  --${role}: light-dark(${light.colors[role]}, ${dark.colors[role]});`,
  );
  declarations.push(
    `  --on-accent: light-dark(${light.onAccent}, ${dark.onAccent});`,
  );
  return `:root {\n${declarations.join("\n")}\n}\n`;
}
