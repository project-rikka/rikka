import type { APIRoute } from "astro";
import { ico } from "../brand/ico";
import { faceIcon } from "../brand/mascot";

const SIZES = [16, 32, 48];

export const GET: APIRoute = async () => {
  const images = await Promise.all(
    SIZES.map(async (size) => ({ size, png: await faceIcon(size, "rounded") })),
  );
  return new Response(new Uint8Array(ico(images)), {
    headers: { "Content-Type": "image/x-icon" },
  });
};
