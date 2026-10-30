export function maxDepth(codes: number[]): number {
  let depth = 0;
  let max = 0;
  for (const code of codes) {
    if (code === 40) {
      depth++;
      if (depth > max) {
        max = depth;
      }
    } else if (code === 41) {
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
