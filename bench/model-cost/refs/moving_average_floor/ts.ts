export function movingAverageFloor(xs: number[], w: number): number[] {
  if (w < 1) {
    throw new Error("w must be at least 1");
  }
  const out: number[] = [];
  for (let i = 0; i + w <= xs.length; i++) {
    let sum = 0;
    for (let j = i; j < i + w; j++) {
      sum += xs[j];
    }
    let q = Math.trunc(sum / w);
    if (q * w > sum) {
      q--;
    }
    out.push(q === 0 ? 0 : q);
  }
  return out;
}
