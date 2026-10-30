export function maxDepth(codes: number[]): number {
  let depth = 0;
  let max = 0;
  for (const c of codes) {
    if (c === 40) {
      depth++;
      if (depth > max) max = depth;
    } else if (c === 41) {
      depth--;
      if (depth < 0) {
        throw new Error("unbalanced parentheses: close without matching open");
      }
    }
  }
  if (depth !== 0) {
    throw new Error("unbalanced parentheses: open never closed");
  }
  return max;
}
