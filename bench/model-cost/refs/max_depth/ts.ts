export function maxDepth(codes: number[]): number {
  let depth = 0;
  let best = 0;
  for (const c of codes) {
    if (c === 40) best = Math.max(best, ++depth);
    else if (c === 41 && --depth < 0) throw new Error("unbalanced");
  }
  if (depth !== 0) throw new Error("unbalanced");
  return best;
}
