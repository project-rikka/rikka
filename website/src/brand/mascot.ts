import { join } from "node:path";
import sharp, { type OutputInfo, type Sharp } from "sharp";

const ILLUSTRATION = join(process.cwd(), "src/assets/rikka.webp");

const FACE = { left: 354, top: 4, width: 508, height: 508 };

const ICON_GROUND = "#ffffff";

const ICON_ROUNDING = 0.2;

export type IconShape = "rounded" | "square";

export function illustration(): Sharp {
  return sharp(ILLUSTRATION);
}

let face: Promise<{ data: Buffer; info: OutputInfo }> | undefined;

function faceCrop() {
  face ??= illustration()
    .extract(FACE)
    .flatten({ background: ICON_GROUND })
    .raw()
    .toBuffer({ resolveWithObject: true });
  return face;
}

export async function faceIcon(
  size: number,
  shape: IconShape,
): Promise<Buffer> {
  const { data, info } = await faceCrop();
  const icon = sharp(data, { raw: info }).resize(size, size, {
    kernel: "lanczos3",
  });
  if (shape === "square") {
    return icon.png().toBuffer();
  }
  const radius = size * ICON_ROUNDING;
  const mask = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}"><rect width="${size}" height="${size}" rx="${radius}"/></svg>`,
  );
  const resized = await icon.raw().toBuffer({ resolveWithObject: true });
  return sharp(resized.data, { raw: resized.info })
    .ensureAlpha()
    .composite([{ input: mask, blend: "dest-in" }])
    .png()
    .toBuffer();
}
