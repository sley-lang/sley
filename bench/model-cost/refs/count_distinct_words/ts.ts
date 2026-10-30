export function countDistinctWords(s: string): number {
  const words = s.match(/[A-Za-z]+/g) ?? [];
  return new Set(words.map((w) => w.toLowerCase())).size;
}
