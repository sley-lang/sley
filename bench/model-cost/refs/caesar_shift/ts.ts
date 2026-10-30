export function caesarShift(s: string, k: number): string {
  const shift = ((k % 26) + 26) % 26;
  let out = "";
  for (const ch of s) {
    const code = ch.charCodeAt(0);
    if (code >= 97 && code <= 122) {
      out += String.fromCharCode(97 + ((code - 97 + shift) % 26));
    } else if (code >= 65 && code <= 90) {
      out += String.fromCharCode(65 + ((code - 65 + shift) % 26));
    } else {
      out += ch;
    }
  }
  return out;
}
