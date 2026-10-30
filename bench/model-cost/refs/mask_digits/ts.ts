export function maskDigits(s: string): string {
  const isDigit = (c: string): boolean => c >= "0" && c <= "9";
  let total = 0;
  for (const ch of s) {
    if (isDigit(ch)) {
      total++;
    }
  }
  let toMask = total - 4;
  let out = "";
  for (const ch of s) {
    if (isDigit(ch) && toMask > 0) {
      out += "*";
      toMask--;
    } else {
      out += ch;
    }
  }
  return out;
}
