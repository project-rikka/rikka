import type { Code } from "astro:components";
import type { ComponentProps } from "astro/types";
import { dark, light } from "virtual:rikka-theme";
import type { Palette, Role } from "../../plugins/theme/palette";

type Theme = Exclude<ComponentProps<typeof Code>["theme"], string | undefined>;

const TOKENS: ReadonlyArray<readonly [Role, readonly string[]]> = [
  ["muted", ["comment", "punctuation"]],
  ["purple", ["keyword", "storage"]],
  [
    "cyan",
    ["keyword.operator", "entity.name.namespace", "entity.name.package"],
  ],
  ["green", ["string", "punctuation.definition.string"]],
  ["magenta", ["constant.character.escape", "constant.other.placeholder"]],
  [
    "orange",
    [
      "constant",
      "support.constant",
      "variable.other.constant",
      "variable.other.enummember",
    ],
  ],
  ["blue", ["entity.name.function", "support.function", "meta.function-call"]],
  [
    "yellow",
    [
      "entity.name.type",
      "entity.name.class",
      "support.class",
      "support.type",
      "entity.other.attribute-name",
      "meta.annotation",
    ],
  ],
  [
    "red",
    [
      "variable.other.property",
      "variable.other.member",
      "support.variable.property",
      "entity.name.tag",
    ],
  ],
  ["subtext", ["variable.parameter"]],
];

function theme(palette: Palette): Theme {
  return {
    name: palette.slug,
    type: palette.variant,
    colors: {
      "editor.background": palette.colors.mantle,
      "editor.foreground": palette.colors.text,
    },
    tokenColors: TOKENS.map(([role, scope]) => ({
      scope: [...scope],
      settings: { foreground: palette.colors[role] },
    })),
  };
}

export const syntaxThemes = { light: theme(light), dark: theme(dark) };
