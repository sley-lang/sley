"""Unit tests and a mock end-to-end run; no network, no key, no spend.

  python -m pytest bench/model-cost/test_model_cost.py   (or python -m unittest)

Toolchain tests skip when gcc, or SLEY_AGENT, is missing."""
import datetime, json, os, shutil, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import accounting, langs, providers, run, tasks  # noqa: E402

PRICES = accounting.load_prices()
LUNA, GROK, DS = (PRICES["models"][m] for m in ("gpt-6-luna", "grok-build-0.1", "deepseek-flash"))


class TaskSet(unittest.TestCase):
    def test_registered_shape(self):
        self.assertEqual(len(tasks.TASKS), 18)
        self.assertEqual(len(tasks.TIERS["small"]), 15)
        groups = [t["group"] for t in tasks.TASKS]
        self.assertEqual((groups.count("existing"), groups.count("new"), groups.count("heldout")), (8, 5, 5))
        self.assertEqual(sum(1 for t in tasks.TASKS if t.get("edit")), 2)

    def test_visible_cases_agree_with_the_oracle(self):
        tasks.check_visible()

    def test_hidden_cases_are_seeded_and_fit_every_language(self):
        for t in tasks.TASKS:
            a, b = tasks.cases(t["name"]), tasks.cases(t["name"])
            self.assertEqual(a, b)
            self.assertEqual(len(a), 128)
            for args, want in a:
                if want != "error":
                    self.assertLess(abs(want), 2 ** 53, t["name"])  # exact in TypeScript numbers
            if t["errors"]:
                self.assertTrue(any(w == "error" for _, w in a), t["name"])

    def test_every_task_has_a_reference_and_signature_in_every_language(self):
        for t in tasks.TASKS:
            for l in langs.LANGS:
                self.assertTrue(os.path.exists(os.path.join(HERE, "refs", t["name"], langs.SRC[l])))
                self.assertIn(langs.fn(t["name"], l), langs.signature(t["name"], l))


class Accounting(unittest.TestCase):
    TOK = {"input": 1068, "cached": 68, "cache_write": 0, "output": 1200}

    def test_per_attempt_costs_match_the_plan(self):
        self.assertAlmostEqual(accounting.cost(LUNA, self.TOK, batch=True), 0.00035, places=5)
        self.assertAlmostEqual(accounting.cost(GROK, self.TOK), 0.00341, places=5)
        sat = datetime.datetime(2026, 11, 7, 12, tzinfo=datetime.timezone.utc)
        self.assertAlmostEqual(accounting.cost(DS, self.TOK, when=sat), 0.00087, places=5)

    def test_deepseek_peak_hours(self):
        tue = lambda h: datetime.datetime(2026, 11, 3, h, tzinfo=datetime.timezone.utc)
        self.assertFalse(accounting.off_peak(DS, tue(2)))
        self.assertFalse(accounting.off_peak(DS, tue(7)))
        self.assertTrue(accounting.off_peak(DS, tue(5)))
        self.assertTrue(accounting.off_peak(DS, tue(12)))
        self.assertFalse(accounting.off_peak(GROK, tue(12)))

    def test_usage_normalization(self):
        o = accounting.normalize_usage("openai", {"input_tokens": 900, "input_tokens_details": {"cached_tokens": 128},
                                                  "output_tokens": 300, "output_tokens_details": {"reasoning_tokens": 100}})
        self.assertEqual(o, {"input": 900, "cached": 128, "cache_write": 0, "output": 300, "reasoning": 100})
        u = {"prompt_tokens": 900, "prompt_tokens_details": {"cached_tokens": 256}, "completion_tokens": 200,
             "completion_tokens_details": {"reasoning_tokens": 100}}
        self.assertEqual(accounting.normalize_usage("xai", u, reasoning_in_output=False)["output"], 300)
        self.assertEqual(accounting.normalize_usage("xai", u, reasoning_in_output=True)["output"], 200)

    def test_worst_case_bounds_any_capped_call(self):
        for p in (LUNA, GROK, DS):
            for b in (False, True):
                full = {"input": 3000, "cached": 0, "cache_write": 0, "output": 2000}
                self.assertGreaterEqual(accounting.worst_case(p, 3000, 2000, batch=b) + 1e-12, accounting.cost(p, full, batch=b))

    def test_guard_refuses_and_persists(self):
        d = tempfile.mkdtemp()
        try:
            path = os.path.join(d, "ledger.jsonl")
            g = accounting.SpendGuard(1.0, path)
            r = g.reserve(0.6)
            with self.assertRaises(accounting.GuardRefused):
                g.reserve(0.5)  # 0.6 held + 0.5 > 1.0
            g.settle(r, 0.4)
            g.settle(g.reserve(0.5), 0.5)
            with self.assertRaises(accounting.GuardRefused):
                g.reserve(0.2)
            self.assertAlmostEqual(accounting.SpendGuard(1.0, path).spent, 0.9)  # resumes
        finally:
            shutil.rmtree(d)


def echo_model(text="```python\ndef f():\n    return 1\n```", finish="stop"):
    return lambda model, messages: (text, finish)


class Providers(unittest.TestCase):
    MSGS = [{"role": "system", "content": "x" * 5000}, {"role": "user", "content": "hello"}]

    def test_openai_batch_round_trip(self):
        t = providers.MockTransport(echo_model(), batch_polls=3)
        p = providers.OpenAIProvider("gpt-6-luna", t, sleep=lambda s: None)
        bid = p.submit_batch([("a", self.MSGS), ("b", self.MSGS)])
        b = p.wait_batch(bid)
        got = p.batch_results(b)
        self.assertEqual(sorted(got), ["a", "b"])
        self.assertTrue(all(r.batch and r.finish == "stop" and "return 1" in r.text for r in got.values()))
        urls = [u for _, u in t.calls]
        self.assertIn("https://api.openai.com/v1/files", urls)
        self.assertIn("https://api.openai.com/v1/batches", urls)
        line = json.loads(t.files[t.batches[bid]["input_file_id"]].decode().splitlines()[0])
        self.assertEqual(line["body"]["max_output_tokens"], 2000)
        self.assertEqual(line["body"]["reasoning"], {"effort": "low"})

    def test_truncation_and_cache_reporting(self):
        t = providers.MockTransport(echo_model(finish="length"))
        p = providers.OpenAIProvider("gpt-6-luna", t)
        r1, r2 = p.complete(self.MSGS), p.complete(self.MSGS)
        self.assertEqual(r1.finish, "length")
        self.assertEqual(r1.usage["input_tokens_details"]["cached_tokens"], 0)
        self.assertGreater(r2.usage["input_tokens_details"]["cached_tokens"], 0)
        x = providers.ChatProvider("xai", "https://api.x.ai", "grok-build-0.1", t).complete(self.MSGS)
        self.assertEqual(x.finish, "length")

    def test_rate_limits_retry_then_fail_as_infrastructure(self):
        class Flaky:
            n = 0

            def request(self, *a):
                Flaky.n += 1
                return (429, b"slow down") if Flaky.n < 3 else (200, json.dumps(
                    {"choices": [{"message": {"content": "ok"}, "finish_reason": "stop"}], "usage": {}}).encode())
        p = providers.ChatProvider("xai", "https://api.x.ai", "m", Flaky(), sleep=lambda s: None)
        self.assertEqual(p.complete(self.MSGS).text, "ok")

        class Down:
            def request(self, *a):
                return 503, b"down"
        with self.assertRaises(providers.InfraError):
            providers.ChatProvider("xai", "https://api.x.ai", "m", Down(), sleep=lambda s: None).complete(self.MSGS)

    def test_paid_needs_the_key(self):
        env = os.environ.pop("XAI_API_KEY", None)
        try:
            with self.assertRaises(SystemExit):
                providers.make("grok-build-0.1", GROK, providers.MockTransport(echo_model()), True, 2000)
        finally:
            if env is not None:
                os.environ["XAI_API_KEY"] = env


class Prompts(unittest.TestCase):
    def test_extract_takes_the_last_block(self):
        self.assertEqual(run.extract("a\n```c\nint x;\n```\nb\n```c\nint y;\n```"), "int y;\n")
        self.assertIsNone(run.extract("```c\nint x;"))

    def test_strip_tests_keeps_dialect_syntax(self):
        src = "import pytest\n\ndef f(n: int) -> int:\n    a, b = 0, 1\n    return a\n\n\ndef test_f():\n    assert f(1) == 0\n"
        out = langs.strip_python_tests(src)
        self.assertIn("a, b = 0, 1", out)
        self.assertNotIn("test_f", out)
        self.assertNotIn("pytest", out)

    def test_edit_prompts_carry_the_starter(self):
        p = run.task_prompt("count_multiples", "rust")
        self.assertIn("x % 2 == 0", p)
        self.assertIn("Result<i64, String>", p)


@unittest.skipUnless(shutil.which("gcc"), "gcc")
class Oracle(unittest.TestCase):
    def test_overfit_program_passes_visible_but_not_hidden(self):
        cheat = ("long long fib(long long n) {\n    if (n == 0) return 0;\n    if (n == 1) return 1;\n"
                 "    if (n == 10) return 55;\n    return 6765;\n}\n")
        d = tempfile.mkdtemp()
        try:
            ok, _ = langs.check("fib", "c", cheat, os.path.join(d, "v"))
            self.assertTrue(ok)
            o = langs.oracle("fib", "c", cheat, os.path.join(d, "o"))
            self.assertLess(o["passed"], o["total"])
            self.assertIsNotNone(o["first_failure"])
        finally:
            shutil.rmtree(d)

    def test_error_convention(self):
        src = open(os.path.join(HERE, "refs", "parse_int", "python.py")).read().replace("ValueError", "KeyError")
        d = tempfile.mkdtemp()
        try:
            o = langs.oracle("parse_int", "python", src, d)
            self.assertEqual(o["first_failure"]["got"], "EXC")  # only ValueError counts as the error
        finally:
            shutil.rmtree(d)


@unittest.skipUnless(os.environ.get("SLEY_AGENT") and shutil.which("gcc"), "SLEY_AGENT and gcc")
class DryRun(unittest.TestCase):
    def run_cli(self, *extra):
        d = tempfile.mkdtemp()
        cmd = [sys.executable, os.path.join(HERE, "run.py"), "--out", d, "--tasks", "fib,parse_int",
               "--langs", "python,sley,c", "--jobs", "4", *extra]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        return d, r

    def test_mock_run_end_to_end(self):
        d, r = self.run_cli()
        try:
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("network calls: 0", r.stderr)
            rows = [json.loads(l) for l in open(os.path.join(d, "results.jsonl"))]
            trials = [x for x in rows if x["kind"] == "trial"]
            self.assertEqual(len(trials), 12)
            self.assertTrue(any(x["kind"] == "batch" for x in rows))
            luna_first = [x for x in rows if x["kind"] == "call" and x["model"] == "gpt-6-luna" and x["attempt"] == 1]
            self.assertTrue(luna_first and all(x["batch"] for x in luna_first))
            self.assertTrue(all(x["attempt"] <= 3 for x in rows if x["kind"] == "call"))
            import summarize
            s = summarize.summarize(d)
            self.assertEqual(s["mode"], "mock")
        finally:
            shutil.rmtree(d)

    def test_guard_stops_the_run(self):
        d, r = self.run_cli("--guard", "0.001", "--models", "grok-build-0.1")
        try:
            self.assertEqual(r.returncode, 0, r.stderr)
            trials = [json.loads(l) for l in open(os.path.join(d, "results.jsonl"))]
            trials = [x for x in trials if x["kind"] == "trial"]
            self.assertTrue(any(x["infra"] and x["infra"].startswith("guard") for x in trials))
            spent = accounting.SpendGuard(1, os.path.join(d, "ledger.jsonl")).spent
            self.assertLessEqual(spent, 0.001)
        finally:
            shutil.rmtree(d)

    def test_guard_above_registration_is_refused(self):
        d, r = self.run_cli("--guard", "50")
        shutil.rmtree(d)
        self.assertNotEqual(r.returncode, 0)


class Projection(unittest.TestCase):
    def test_lean_budget(self):
        small = run.projection(PRICES, ["gpt-6-luna", "grok-build-0.1"], "small", 1)
        full = run.projection(PRICES, ["gpt-6-luna", "grok-build-0.1"], "full", 3)
        self.assertEqual(small["trials_per_model"], 90)
        self.assertEqual(full["trials_per_model"], 324)
        self.assertLess(small["ceiling"], 2.0)
        self.assertLess(full["ceiling"], 10.0)


if __name__ == "__main__":
    unittest.main()
