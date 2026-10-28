export function inversions(xs: number[]): number {
  let count = 0;
  for (let i = 0; i < xs.length; i++) {
    const x = xs[i];
    for (let j = i + 1; j < xs.length; j++) {
      if (xs[j] < x) {
        count++;
      }
    }
  }
  return count;
}
