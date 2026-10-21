export function withdraw(balance: number, amount: number): number {
  if (amount <= 0) {
    throw new RangeError("amount must be positive");
  }
  if (amount > balance) {
    throw new RangeError("insufficient funds");
  }
  return balance - amount;
}
