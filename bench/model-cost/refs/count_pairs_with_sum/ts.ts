export function countPairsWithSum(xs: number[], target: number): number {
  let count = 0;
  for (let i = 0; i < xs.length; i++) {
    for (let j = i + 1; j < xs.length; j++) {
      if (xs[i] + xs[j] === target) {
        count++;
      }
    }
  }
  return count;
}
