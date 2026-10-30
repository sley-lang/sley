export function runLengthEncode(s: string): string {
  let out = "";
  let i = 0;
  while (i < s.length) {
    let j = i;
    while (j < s.length && s[j] === s[i]) {
      j++;
    }
    out += s[i] + String(j - i);
    i = j;
  }
  return out;
}
