export function runLengthDecode(s: string): string {
  const isDigit = (c: string): boolean => c >= "0" && c <= "9";
  const groups: Array<[string, number]> = [];
  let total = 0;
  let i = 0;
  while (i < s.length) {
    const ch = s[i];
    const code = ch.charCodeAt(0);
    if (isDigit(ch) || code < 32 || code > 126) {
      throw new Error("expected a non-digit printable character");
    }
    i++;
    if (i >= s.length || !isDigit(s[i])) {
      throw new Error("missing count");
    }
    if (s[i] === "0") {
      throw new Error("count has a leading zero");
    }
    let j = i;
    while (j < s.length && isDigit(s[j])) {
      j++;
    }
    const digits = s.slice(i, j);
    if (digits.length > 4) {
      throw new Error("decoded string too long");
    }
    const count = Number(digits);
    total += count;
    if (total > 1000) {
      throw new Error("decoded string too long");
    }
    groups.push([ch, count]);
    i = j;
  }
  let out = "";
  for (const [ch, count] of groups) {
    out += ch.repeat(count);
  }
  return out;
}
