export function mostCommonLetter(s: string): string {
  const counts = new Array<number>(26).fill(0);
  for (const ch of s.toLowerCase()) {
    if (ch >= "a" && ch <= "z") {
      counts[ch.charCodeAt(0) - 97]++;
    }
  }
  let best = -1;
  for (let i = 0; i < 26; i++) {
    if (counts[i] > 0 && (best === -1 || counts[i] > counts[best])) {
      best = i;
    }
  }
  if (best === -1) {
    throw new Error("no ASCII letters");
  }
  return String.fromCharCode(97 + best);
}
