/// <reference types="astro/client" />

declare module "virtual:rikka-theme" {
  import type { Palette } from "../plugins/theme/palette";
  export const light: Palette;
  export const dark: Palette;
}

declare module "virtual:rikka-theme.css";

declare module "workspace:*" {
  const text: string;
  export default text;
}
