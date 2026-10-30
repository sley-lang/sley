export function parseClockSeconds(s: string): number {
  if (!/^[0-9]{2}:[0-9]{2}:[0-9]{2}$/.test(s)) {
    throw new Error("invalid time format");
  }
  const hh = Number(s.slice(0, 2));
  const mm = Number(s.slice(3, 5));
  const ss = Number(s.slice(6, 8));
  if (hh > 23 || mm > 59 || ss > 59) {
    throw new Error("time value out of range");
  }
  return hh * 3600 + mm * 60 + ss;
}
