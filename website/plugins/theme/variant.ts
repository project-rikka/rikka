export type Variant = "light" | "dark";

export function isVariant(value: unknown): value is Variant {
  return value === "light" || value === "dark";
}
