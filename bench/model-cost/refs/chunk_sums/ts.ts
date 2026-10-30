export function chunkSums(xs: number[], size: number): number[] {
  if (size < 1) {
    throw new Error("size must be at least 1");
  }
  const sums: number[] = [];
  for (let i = 0; i < xs.length; i += size) {
    let sum = 0;
    for (let j = i; j < Math.min(i + size, xs.length); j++) {
      sum += xs[j];
    }
    sums.push(sum);
  }
  return sums;
}
