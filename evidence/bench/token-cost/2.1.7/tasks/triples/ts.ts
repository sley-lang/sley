export function triples(n: number): number {
  let count = 0;
  for (let c = 1; c <= n; c++) {
    for (let b = 1; b < c; b++) {
      for (let a = 1; a < b; a++) {
        if (a * a + b * b === c * c) {
          count++;
        }
      }
    }
  }
  return count;
}
