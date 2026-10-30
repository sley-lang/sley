export function bracketDepth(s: string): number {
  const pairs: Record<string, string> = { ")": "(", "]": "[", "}": "{" };
  const stack: string[] = [];
  let maxDepth = 0;
  for (const ch of s) {
    if (ch === "(" || ch === "[" || ch === "{") {
      stack.push(ch);
      maxDepth = Math.max(maxDepth, stack.length);
    } else if (ch === ")" || ch === "]" || ch === "}") {
      if (stack.length === 0 || stack[stack.length - 1] !== pairs[ch]) {
        throw new Error("mismatched bracket");
      }
      stack.pop();
    }
  }
  if (stack.length !== 0) {
    throw new Error("unclosed bracket");
  }
  return maxDepth;
}
