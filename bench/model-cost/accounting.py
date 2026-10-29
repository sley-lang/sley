"""Billed-usage accounting and the spend guard.

Costs come from the usage object each API returns for the call (what the
provider bills), priced with prices.json. Local token counts are never used
for spend. The guard reserves a call's worst case before it is sent and
settles the billed cost afterwards; the ledger is persisted so an interrupted
run resumes with its spend intact.
"""
import datetime, json, os, threading

HERE = os.path.dirname(os.path.abspath(__file__))
M = 1_000_000


def load_prices(path=None):
    return json.load(open(path or os.path.join(HERE, "prices.json")))


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc)


def off_peak(model_price, when):
    """DeepSeek's off-peak table applies outside 01-04 and 06-10 UTC, Monday to Friday."""
    if "off_peak" not in model_price:
        return False
    if when.weekday() >= 5:
        return True
    return not (1 <= when.hour < 4 or 6 <= when.hour < 10)


def rates(price, batch=False, when=None):
    """(input, cached input, cache write, output) dollars per token for one call."""
    p = price
    if when is not None and off_peak(price, when):
        p = dict(price, **price["off_peak"])
    k = price.get("batch_discount", 1.0) if batch else 1.0
    return (p["input"] * k / M, p["cached_input"] * k / M, p.get("cache_write", p["input"]) * k / M, p["output"] * k / M)


def normalize_usage(provider, usage, reasoning_in_output=True):
    """Map a provider usage object to billed token counts.

    input: all prompt tokens (cached included); cached: prompt tokens billed at
    the cached rate; cache_write: prompt tokens billed at the cache-write rate;
    output: completion tokens billed at the output rate (reasoning included);
    reasoning: the reasoning part of output (reported, already counted)."""
    usage = usage or {}
    if provider == "openai":
        d = usage.get("input_tokens_details") or {}
        o = usage.get("output_tokens_details") or {}
        return {"input": int(usage.get("input_tokens", 0)), "cached": int(d.get("cached_tokens", 0)),
                "cache_write": int(d.get("cache_write_tokens", d.get("cache_creation_tokens", 0)) or 0),
                "output": int(usage.get("output_tokens", 0)), "reasoning": int(o.get("reasoning_tokens", 0))}
    # chat-completions shape (xAI, DeepSeek)
    d = usage.get("prompt_tokens_details") or {}
    o = usage.get("completion_tokens_details") or {}
    cached = int(d.get("cached_tokens", usage.get("prompt_cache_hit_tokens", 0)) or 0)
    completion = int(usage.get("completion_tokens", 0))
    reasoning = int(o.get("reasoning_tokens", 0) or 0)
    # Some APIs report reasoning outside completion_tokens; then it bills on top.
    output = completion if reasoning_in_output else completion + reasoning
    return {"input": int(usage.get("prompt_tokens", 0)), "cached": cached, "cache_write": 0,
            "output": output, "reasoning": reasoning}


def cost(price, tokens, batch=False, when=None):
    i, c, w, o = rates(price, batch, when)
    uncached = max(tokens["input"] - tokens["cached"] - tokens["cache_write"], 0)
    return uncached * i + tokens["cached"] * c + tokens["cache_write"] * w + tokens["output"] * o


def worst_case(price, prompt_tokens, max_output, batch=False):
    """The most a call can bill: every prompt token at the higher of the input
    and cache-write rate (peak rate where there is one) and the full output cap."""
    i, _, w, o = rates(price, batch)
    return prompt_tokens * max(i, w) + max_output * o


def estimate_tokens(text):
    """A deliberately high prompt-size estimate for reservations only (about 3
    characters per token); billing uses the API's usage."""
    return len(text) // 3 + 16


class GuardRefused(Exception):
    pass


class SpendGuard:
    """Stops a run before spend could pass `limit` dollars."""

    def __init__(self, limit, ledger_path):
        self.limit = float(limit)
        self.path = ledger_path
        self.lock = threading.Lock()
        self.spent = 0.0
        self.reserved = {}
        self.next_id = 0
        if os.path.exists(ledger_path):
            for line in open(ledger_path):
                e = json.loads(line)
                if e["kind"] == "settle":
                    self.spent += e["cost"]

    def _log(self, entry):
        entry["utc"] = utc_now().isoformat()
        with open(self.path, "a") as f:
            f.write(json.dumps(entry) + "\n")

    def reserve(self, amount, label=""):
        with self.lock:
            held = sum(self.reserved.values())
            if self.spent + held + amount > self.limit:
                self._log({"kind": "refuse", "amount": amount, "label": label, "spent": self.spent, "held": held})
                raise GuardRefused(f"spend guard: ${self.spent:.4f} spent + ${held:.4f} held + ${amount:.4f} "
                                   f"for {label} would pass ${self.limit:.2f}")
            rid = self.next_id
            self.next_id += 1
            self.reserved[rid] = amount
            self._log({"kind": "reserve", "id": rid, "amount": amount, "label": label})
            return rid

    def settle(self, rid, actual, label=""):
        with self.lock:
            self.reserved.pop(rid, None)
            self.spent += actual
            self._log({"kind": "settle", "id": rid, "cost": actual, "label": label, "spent": self.spent})

    def release(self, rid, label=""):
        """A reservation that billed nothing (refused before sending)."""
        with self.lock:
            self.reserved.pop(rid, None)
            self._log({"kind": "release", "id": rid, "label": label})
