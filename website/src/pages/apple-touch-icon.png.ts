import type { APIRoute } from "astro";
import { faceIcon } from "../brand/mascot";

const SIZE = 180;

export const GET: APIRoute = async () =>
  new Response(new Uint8Array(await faceIcon(SIZE, "square")), {
    headers: { "Content-Type": "image/png" },
  });
