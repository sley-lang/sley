"""Help-card accounting for the token-cost agent loop (T6).

A card is the one-time help text an agent reads for a Sley form before the
first `try` of a session: `help python` for the Python dialect, `help card`
for familiar syntax, `help` + `help structured` for AF1-X. Python and the
other benchmark languages read no card. The assumption, stated in the README
and in every with-card table (counted explicitly, never silently):

  the agent reads the form's card once per task session -- one
  (task, kind, form) loop run -- and never re-reads it on a retry
  (fix -> run -> read). Aggregation therefore adds one card per session
  row, never per tool call or per retry within a session.

run_loop.py measures each form's card once with the same sley-agent binary
that ran the loop and records the per-tokenizer counts in every loop.json
row's `card` field. With-card reporting uses recorded cards only: nothing is
re-measured from a different binary. Legacy loop.json records that predate
card accounting have no `card` field, so their Sley with-card totals are
reported as unavailable, never defaulted to 0 and never counted per retry.
"""

# Help topics each Sley form reads once per session (run_loop.py also uses
# this to run `sley-agent help <topic>` when measuring a card).
SLEY_FORM_CARDS = {
    "sley_py": ["python"],
    "sley_py_inline": ["python"],
    "sley_fam": ["card"],
    "sley_fam_decl": ["card"],
    "sley_af1": ["", "structured"],
}

# Forms a model already knows; they read no card, so their card is 0 by
# definition, with or without a recorded `card` field.
LANG_FORMS = frozenset({"python", "c", "rust", "go", "ts"})


def card_tokens(row, enc, form):
    """The card tokens of one session row (one task session), or None.

    Non-Sley forms read no card: 0 by definition. A Sley row without a
    recorded card (a legacy loop.json predating card accounting) is None:
    its with-card total is unavailable, never invented from another source.
    """
    if form in LANG_FORMS:
        return 0
    card = row.get("card") if isinstance(row, dict) else None
    if not isinstance(card, dict) or enc not in card:
        return None
    value = card[enc]
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"bad {form} card field for {enc}: {value!r}")
    return value


def with_card_totals(rows, enc, form):
    """(written, read, total, tool calls, card) over one form's session rows.

    Adds exactly one card per row -- one per task session, never per retry.
    Returns None when any Sley row lacks a recorded card (legacy record);
    the totals are then unavailable, not zero.
    """
    if form not in LANG_FORMS and any(card_tokens(r, enc, form) is None for r in rows):
        return None
    w = sum(r["write"] for r in rows)
    rd = sum(r["read"] for r in rows)
    card = sum(card_tokens(r, enc, form) for r in rows)
    calls = sum(r["tool_calls"] for r in rows)
    return (w, rd, w + rd + card, calls, card)


def measure_cards(sley, forms, encodings, toks, run):
    """Each form's one-time help card, measured once with the binary that runs
    the loop (cards.py: read once per task session, never per retry). The text
    is kept in full in cards.json; the per-tokenizer counts go into every
    loop.json row's `card` field."""
    measured = {}
    for form in forms:
        topics = SLEY_FORM_CARDS.get(form)
        if topics is None:  # Python and the other languages: no guide, no card
            measured[form] = {"help": None, "text": None, "tokens": {n: 0 for n in encodings}}
            continue
        text, codes = "", []
        for t in topics:
            r = run([sley, "help"] + ([t] if t else []), capture_output=True, text=True)
            if r.returncode != 0:
                raise RuntimeError(f"help {t!r} failed ({r.returncode}): {r.stderr or r.stdout}")
            codes.append(r.returncode); text += r.stdout + r.stderr
        measured[form] = {"help": " + ".join(f"help {t}" if t else "help" for t in topics),
                          "text": text, "returncode": codes, "tokens": toks(text)}
    return measured

