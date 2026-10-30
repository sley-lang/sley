export function formatThousands(n: number): string {
  const digits = String(Math.abs(n));
  let out = "";
  for (let i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 === 0) {
      out += ",";
    }
    out += digits[i];
  }
  return n < 0 ? "-" + out : out;
}
