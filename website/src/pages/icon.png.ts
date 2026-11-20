import type { APIRoute } from "astro";
import { faceIcon } from "../brand/mascot";

const SIZE = 192;

export const GET: APIRoute = async () =>
  new Response(new Uint8Array(await faceIcon(SIZE, "rounded")), {
    headers: { "Content-Type": "image/png" },
  });
