"""Offline unit tests for the token-cost help-card aggregation (T6): one card
per task session, never per retry; legacy records without `card` fields report
their Sley with-card totals as unavailable, never zero."""
import sys, unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))  # bench/token-cost
import cards  # cards.py: measured cards live in loop.json/cards.json; no binary or network here


CARD = {"o200k_base": 100, "cl100k_base": 96}


def row(task, form="sley_py", kind="logic", w=10, rd=20, calls=2, card="absent"):
    r = {"task": task, "kind": kind, "form": form, "enc": "o200k_base",
         "write": w, "read": rd, "total": w + rd, "tool_calls": calls}
    if card != "absent":
        r["card"] = card
    return r


class CardAccounting(unittest.TestCase):
    def test_card_added_once_per_task_session(self):
        # Two task sessions; each scenario row is one session that read the
        # card once, whatever the number of fix-retry cycles inside it.
        w, rd, total, calls, card = cards.with_card_totals([row("a", card=CARD), row("b", card=CARD)], "o200k_base", "sley_py")
        self.assertEqual((w, rd, calls), (20, 40, 4))
        self.assertEqual(card, 200)
        self.assertEqual(total, 60 + 200)

    def test_card_is_never_counted_per_retry(self):
        # Tries stay inside a session row: a fix-and-rerun (logic) session is
        # still one (task, kind, form) run and pays one card, not one per try.
        # One without-retry session plus one with a retry pays two cards.
        sessions = [row("a", kind="clean", card=CARD), row("a", kind="logic", card=CARD)]
        self.assertEqual(cards.with_card_totals(sessions, "o200k_base", "sley_py")[4], 200)
        # The card never scales with the tool calls inside a session either.
        retried = [row("a", kind="logic", calls=28, card=CARD)]  # a full fix cycle in one session
        self.assertEqual(cards.with_card_totals(retried, "o200k_base", "sley_py")[4], 100)

    def test_forms_without_a_card_read_zero_by_definition(self):
        for form in ("python", "c", "rust", "go", "ts"):
            self.assertEqual(cards.card_tokens(row("a", form=form), "o200k_base", form), 0)
            self.assertEqual(cards.with_card_totals([row("a", form=form)], "o200k_base", form),
                             (10, 20, 30, 2, 0))
        # ... whether or not the record carries a `card` field at all
        self.assertEqual(cards.card_tokens(row("a", form="python", card=CARD), "o200k_base", "python"), 0)

    def test_legacy_records_lacking_card_fields_are_unavailable_not_zero(self):
        legacy = [row("a"), row("b")]  # pre-T6 loop.json rows: no `card` field
        self.assertIsNone(cards.card_tokens(legacy[0], "o200k_base", "sley_py"))
        self.assertIsNone(cards.card_tokens(legacy[0], "cl100k_base", "sley_fam"))
        self.assertIsNone(cards.with_card_totals(legacy, "o200k_base", "sley_py"))
        self.assertIsNone(cards.with_card_totals(legacy, "cl100k_base", "sley_af1"))

    def test_mixed_legacy_and_measured_rows_cannot_be_aggregated(self):
        rows = [row("a", card=CARD), row("b")]  # one recorded card, one legacy row
        self.assertIsNone(cards.with_card_totals(rows, "o200k_base", "sley_py"))

    def test_malformed_card_field_values(self):
        for bad in (1.5, True, -1):
            with self.assertRaises(ValueError):
                cards.card_tokens(row("a", card={"o200k_base": bad}), "o200k_base", "sley_py")
        # A card missing the requested tokenizer is unavailable for it, not an error
        self.assertIsNone(cards.card_tokens(row("a", card={"cl100k_base": 5}), "o200k_base", "sley_py"))
        # A non-mapping `card` field reads as a legacy record, never as 0
        self.assertIsNone(cards.card_tokens(row("a", card=100), "o200k_base", "sley_py"))

    def test_card_topics_per_sley_form(self):
        self.assertEqual(cards.SLEY_FORM_CARDS, {
            "sley_py": ["python"], "sley_py_inline": ["python"],
            "sley_fam": ["card"], "sley_fam_decl": ["card"],
            "sley_af1": ["", "structured"]})
        # Unknown/typo form names are treated as Sley: they need a recorded
        # card or the aggregation stays unavailable, fail-closed.
        self.assertIsNone(cards.card_tokens(row("a"), "o200k_base", "sley_fem"))


class CardCollection(unittest.TestCase):
    def test_bare_help_has_no_empty_argument(self):
        from types import SimpleNamespace
        seen = []
        def run(argv, **kwargs):
            seen.append(argv)
            return SimpleNamespace(returncode=0, stdout="card", stderr="")
        result = cards.measure_cards("sley", ["sley_af1"], ["o200k_base"], lambda text: {"o200k_base": len(text)}, run)
        self.assertEqual(seen, [["sley", "help"], ["sley", "help", "structured"]])
        self.assertEqual(result["sley_af1"]["tokens"]["o200k_base"], 8)

    def test_help_failure_is_not_counted_as_a_card(self):
        from types import SimpleNamespace
        def run(argv, **kwargs):
            return SimpleNamespace(returncode=2, stdout="error", stderr="failed")
        with self.assertRaises(RuntimeError):
            cards.measure_cards("sley", ["sley_py"], ["o200k_base"], lambda text: {}, run)


if __name__ == "__main__":
    unittest.main()
