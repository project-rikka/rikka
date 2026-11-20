const ARMS = 6;

const STROKES: ReadonlyArray<readonly [number, number, number, number]> = [
  [0, 0, 0, -13],
  [0, -7.5, -4.5, -11],
  [0, -7.5, 4.5, -11],
];

function rotate(x: number, y: number, turn: number): string {
  const angle = (turn * 2 * Math.PI) / ARMS;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const round = (value: number) => Number(value.toFixed(2));
  return `${round(x * cos - y * sin)} ${round(x * sin + y * cos)}`;
}

export const markPath = Array.from({ length: ARMS }, (_, turn) =>
  STROKES.map(
    ([x1, y1, x2, y2]) => `M${rotate(x1, y1, turn)}L${rotate(x2, y2, turn)}`,
  ).join(""),
).join("");

export const MARK_EXTENT = 32;

export const markViewBox = `${-MARK_EXTENT / 2} ${-MARK_EXTENT / 2} ${MARK_EXTENT} ${MARK_EXTENT}`;
