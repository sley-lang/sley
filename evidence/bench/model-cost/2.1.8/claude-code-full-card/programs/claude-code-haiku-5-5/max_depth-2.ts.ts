export function maxDepth(codes: number[]): number {
  const OPEN = 40;
  const CLOSE = 41;
  let depth = 0;
  let max = 0;
  for (const code of codes) {
    if (code === OPEN) {
      depth++;
      if (depth > max) max = depth;
    } else if (code === CLOSE) {
      if (depth === 0) {
        throw new Error("unbalanced parentheses: close without matching open");
      }
      depth--;
    }
  }
  if (depth !== 0) {
    throw new Error("unbalanced parentheses: open never closed");
  }
  return max;
}
