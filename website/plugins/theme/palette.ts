import { parse } from "smol-toml";
import { isVariant, type Variant } from "./variant";

export const ROLES = [
  "crust",
  "mantle",
  "base",
  "surface",
  "overlay",
  "text",
  "subtext",
  "muted",
  "border",
  "cursor",
  "highlight",
  "link",
  "accent",
  "red",
  "orange",
  "yellow",
  "green",
  "cyan",
  "blue",
  "purple",
  "magenta",
  "error",
  "warning",
  "success",
  "info",
] as const;

export type Role = (typeof ROLES)[number];

export interface Palette {
  readonly slug: string;
  readonly variant: Variant;
  readonly colors: Readonly<Record<Role, string>>;
  readonly onAccent: string;
}

const HEX = /^#[0-9a-f]{6}$/i;

const AA = 4.5;

export function readPalette(source: string, slug: string): Palette {
  const spec = parse(source);
  const variant = spec["variant"];
  if (!isVariant(variant)) {
    throw new Error(`theme ${slug}: variant must be light or dark`);
  }
  const table = spec["colors"];
  if (typeof table !== "object" || table === null || Array.isArray(table)) {
    throw new Error(`theme ${slug}: missing [colors]`);
  }
  const entries = table as Record<string, unknown>;
  const colors = Object.fromEntries(
    ROLES.map((role) => {
      const value = entries[role];
      if (typeof value !== "string" || !HEX.test(value)) {
        throw new Error(`theme ${slug}: colors.${role} must be #rrggbb`);
      }
      return [role, value.toLowerCase()];
    }),
  ) as Record<Role, string>;
  return {
    slug,
    variant,
    colors,
    onAccent: contrastingText(colors.accent, colors.crust, colors.text),
  };
}

function contrastingText(fill: string, a: string, b: string): string {
  const ratioA = contrastRatio(fill, a);
  const ratioB = contrastRatio(fill, b);
  const [best, bestRatio] = ratioA >= ratioB ? [a, ratioA] : [b, ratioB];
  if (bestRatio >= AA) {
    return best;
  }
  return contrastRatio(fill, "#000000") >= contrastRatio(fill, "#ffffff")
    ? "#000000"
    : "#ffffff";
}

function contrastRatio(a: string, b: string): number {
  const [lighter, darker] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (lighter! + 0.05) / (darker! + 0.05);
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((offset) => {
    const channel = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}
