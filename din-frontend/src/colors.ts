// Value → color scale. Low values are cool (indigo), high values warm (amber).

export function colorForValue(value: number): string {
  if (value <= 0) return '#3b4252';
  const t = Math.min(1, value / 500);
  const low = [94, 129, 244]; // indigo
  const mid = [52, 211, 153]; // emerald
  const high = [251, 191, 36]; // amber
  const from = t < 0.5 ? low : mid;
  const to = t < 0.5 ? mid : high;
  const tt = t < 0.5 ? t * 2 : (t - 0.5) * 2;
  const c = from.map((x, i) => Math.round(x + (to[i] - x) * tt));
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
}

/** Node scale factor driven by value (larger value = slightly larger node). */
export function scaleForValue(value: number): number {
  return Math.min(1.3, Math.max(0.85, 0.85 + value / 1400));
}

export function shorten(id: string, n = 10): string {
  return id.length > n * 2 ? `${id.slice(0, n)}…${id.slice(-n)}` : id;
}
