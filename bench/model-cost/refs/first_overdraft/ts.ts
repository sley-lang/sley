export function firstOverdraft(start: number, xs: number[]): number {
  let balance = start;
  for (let i = 0; i < xs.length; i++) {
    balance += xs[i];
    if (balance < 0) {
      return i;
    }
  }
  return -1;
}
