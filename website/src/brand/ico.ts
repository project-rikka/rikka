export interface IconImage {
  readonly size: number;
  readonly png: Buffer;
}

const HEADER = 6;
const ENTRY = 16;

export function ico(images: readonly IconImage[]): Buffer {
  const directory = Buffer.alloc(HEADER + ENTRY * images.length);
  directory.writeUInt16LE(0, 0);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(images.length, 4);
  let offset = directory.length;
  images.forEach((image, index) => {
    if (image.size < 1 || image.size > 256) {
      throw new Error(`ico: ${image.size} px is outside 1–256`);
    }
    const entry = HEADER + ENTRY * index;
    const edge = image.size === 256 ? 0 : image.size;
    directory.writeUInt8(edge, entry);
    directory.writeUInt8(edge, entry + 1);
    directory.writeUInt8(0, entry + 2);
    directory.writeUInt8(0, entry + 3);
    directory.writeUInt16LE(1, entry + 4);
    directory.writeUInt16LE(32, entry + 6);
    directory.writeUInt32LE(image.png.length, entry + 8);
    directory.writeUInt32LE(offset, entry + 12);
    offset += image.png.length;
  });
  return Buffer.concat([directory, ...images.map((image) => image.png)]);
}
