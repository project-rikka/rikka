import type { APIRoute } from "astro";
import sharp from "sharp";
import { light } from "virtual:rikka-theme";
import { CARD } from "../brand/card";
import { MARK_EXTENT, markPath } from "../brand/mark";
import { illustration } from "../brand/mascot";
import { site } from "../site";

const MARGIN = 80;

const FIGURE = { center: 900, scale: 1.7, top: 0.03 };

const GLOW = { y: 0.45, radius: 0.75, opacity: 0.2 };

const FROST = { radius: 0.9, opacity: 0.22 };

const FLAKE = { y: 0.42, size: 672, opacity: 0.22 };

const TITLE = { inset: 6, baseline: 316, size: 128, tracking: -5 };

const TAGLINE = {
  lines: [
    "Wayland compositor and desktop on NixOS,",
    "built for clean UI and usability.",
  ],
  baseline: 394,
  leading: 42,
  size: 30,
};

async function portrait() {
  const scaled = await illustration()
    .resize({ height: Math.round(CARD.height * FIGURE.scale) })
    .raw()
    .toBuffer({ resolveWithObject: true });
  return sharp(scaled.data, { raw: scaled.info })
    .extract({
      left: 0,
      top: Math.round(scaled.info.height * FIGURE.top),
      width: scaled.info.width,
      height: CARD.height,
    })
    .raw()
    .toBuffer({ resolveWithObject: true });
}

function backdrop(): string {
  const { colors } = light;
  const { width, height } = CARD;
  const flakeScale = FLAKE.size / MARK_EXTENT;
  const tagline = TAGLINE.lines
    .map(
      (line, index) =>
        `<text x="${MARGIN}" y="${TAGLINE.baseline + index * TAGLINE.leading}" font-family="Inter" font-size="${TAGLINE.size}" fill="${colors.subtext}">${line}</text>`,
    )
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">
    <defs>
      <radialGradient id="glow">
        <stop offset="0" stop-color="${colors.accent}" stop-opacity="${GLOW.opacity}"/>
        <stop offset="1" stop-color="${colors.accent}" stop-opacity="0"/>
      </radialGradient>
      <radialGradient id="frost">
        <stop offset="0" stop-color="${colors.cyan}" stop-opacity="${FROST.opacity}"/>
        <stop offset="1" stop-color="${colors.cyan}" stop-opacity="0"/>
      </radialGradient>
    </defs>
    <rect width="${width}" height="${height}" fill="${colors.base}"/>
    <circle cx="0" cy="0" r="${height * FROST.radius}" fill="url(#frost)"/>
    <circle cx="${FIGURE.center}" cy="${height * GLOW.y}" r="${height * GLOW.radius}" fill="url(#glow)"/>
    <path d="${markPath}" transform="translate(${FIGURE.center} ${height * FLAKE.y}) scale(${flakeScale})" fill="none" stroke="${colors.accent}" stroke-opacity="${FLAKE.opacity}" stroke-width="${1 / flakeScale}" stroke-linecap="round"/>
    <text x="${MARGIN - TITLE.inset}" y="${TITLE.baseline}" font-family="Inter" font-size="${TITLE.size}" font-weight="600" letter-spacing="${TITLE.tracking}" fill="${colors.text}">${site.name}</text>
    ${tagline}
  </svg>`;
}

export const GET: APIRoute = async () => {
  const figure = await portrait();
  const png = await sharp(Buffer.from(backdrop()))
    .composite([
      {
        input: figure.data,
        raw: figure.info,
        left: Math.round(FIGURE.center - figure.info.width / 2),
        top: 0,
      },
    ])
    .png()
    .toBuffer();
  return new Response(new Uint8Array(png), {
    headers: { "Content-Type": "image/png" },
  });
};
