"""Unit tests and a mock end-to-end run; no network, no key, no spend.

  python -m pytest bench/model-cost/test_model_cost.py   (or python -m unittest)

Toolchain tests skip when gcc, or SLEY_AGENT, is missing."""
import datetime, json, os, shutil, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import accounting, intervals, langs, providers, run, tasks, tasks_v2  # noqa: E402

PRICES = accounting.load_prices()
LUNA, GROK, DS, HAIKU = (PRICES["models"][m] for m in ("gpt-6-luna", "grok-build-0.1", "deepseek-flash",
                                                         "claude-haiku-5-5"))


class TaskSet(unittest.TestCase):
    def test_registered_shape(self):
        self.assertEqual(len(tasks.TASKS), 18)
        self.assertEqual(len(tasks.TIERS["small"]), 15)
        groups = [t["group"] for t in tasks.TASKS]
        self.assertEqual((groups.count("existing"), groups.count("new"), groups.count("heldout")), (8, 5, 5))
        self.assertEqual(sum(1 for t in tasks.TASKS if t.get("edit")), 2)

    def test_visible_cases_agree_with_the_oracle(self):
        tasks.check_visible()

    def test_collatz_limit_two_is_one(self):
        # 1 is the only start below 2; it takes 0 steps and is still the answer
        self.assertEqual(tasks.reference("collatz", [2]), 1)
        self.assertEqual(tasks.reference("collatz", [1]), 0)

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


class TaskSetV2(unittest.TestCase):
    def test_authored_tasks_load_unedited(self):
        authored = json.load(open(tasks_v2.AUTHORED))
        self.assertEqual(len(tasks_v2.TASKS), 24)
        self.assertEqual([t["name"] for t in authored], [t["name"] for t in tasks_v2.TASKS])
        self.assertEqual(tasks_v2.AMENDMENTS, {})
        self.assertFalse(set(tasks_v2.BY_NAME) & set(tasks.BY_NAME), "v2 names must not shadow v1")

    def test_authored_examples_agree_with_the_oracle(self):
        self.assertEqual(tasks_v2.check_visible(), [])

    def test_hidden_cases_are_seeded_in_bounds_and_mixed(self):
        for t in tasks_v2.TASKS:
            cs = tasks_v2.cases(t["name"])
            self.assertEqual(len(cs), 128)
            self.assertEqual(cs, tasks_v2.cases(t["name"]))
            self.assertNotEqual(cs, tasks_v2.cases(t["name"], seed=219))
            for args, want in cs:
                self.assertTrue(tasks_v2.in_bounds(t["name"], args), (t["name"], args))
            outs = {tasks_v2.show(w) for _, w in cs}
            self.assertGreater(len(outs), 1, t["name"])
            if t["errors"]:
                errs = sum(1 for _, w in cs if w == "error")
                self.assertTrue(8 <= errs <= 100, (t["name"], errs))
            else:
                self.assertNotIn("ERR", outs, t["name"])

    def test_show_is_one_compact_line(self):
        self.assertEqual(tasks_v2.show(-3), "-3")
        self.assertEqual(tasks_v2.show([1, -2]), "[1,-2]")
        self.assertEqual(tasks_v2.show([]), "[]")
        self.assertEqual(tasks_v2.show('a"b\\c'), '"a\\"b\\\\c"')
        self.assertEqual(tasks_v2.show("error"), "ERR")

    def test_oracle_spot_checks(self):
        r = tasks_v2.reference
        self.assertEqual(r("covered_length", [[0, 5, 5, 7, 1, 2]]), 7)
        self.assertEqual(r("count_by_remainder", [[-1, 2, 3], 3]), [1, 0, 2])
        self.assertEqual(r("moving_average_floor", [[-1, 0], 2]), [-1])
        self.assertEqual(r("mask_digits", ["a12345"]), "a*2345")
        self.assertEqual(r("caesar_shift", ["Zz", -1000000]), "Ll")
        self.assertEqual(r("to_roman", [3999]), "MMMCMXCIX")
        with self.assertRaises(tasks_v2.Error):
            r("run_length_decode", ["a1000b1"])


class Accounting(unittest.TestCase):
    TOK = {"input": 1068, "cached": 68, "cache_write": 0, "output": 1200}

    def test_per_attempt_costs_match_the_plan(self):
        self.assertAlmostEqual(accounting.cost(LUNA, self.TOK, batch=True), 0.00035, places=5)
        self.assertAlmostEqual(accounting.cost(GROK, self.TOK), 0.00341, places=5)
        # Haiku 5.5 on the Batch API: 1,000 in at $0.05, 68 cache reads at $0.005, 1,200 out at $0.25 per MTok
        self.assertAlmostEqual(accounting.cost(HAIKU, self.TOK, batch=True), 0.00035034, places=8)
        # standard rate: $0.10 / $0.01 / $0.50
        self.assertAlmostEqual(accounting.cost(HAIKU, self.TOK), 0.00070068, places=8)
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

    def test_anthropic_usage_normalization(self):
        u = {"input_tokens": 40, "cache_creation_input_tokens": 600, "cache_read_input_tokens": 0, "output_tokens": 900,
             "cache_creation": {"ephemeral_5m_input_tokens": 600, "ephemeral_1h_input_tokens": 0}}
        o = accounting.normalize_usage("anthropic", u)
        self.assertEqual(o, {"input": 640, "cached": 0, "cache_write": 600, "cache_write_1h": 0, "output": 900,
                             "reasoning": 0})
        # 40 x $0.10 + 600 x $0.125 + 900 x $0.50 per MTok
        self.assertAlmostEqual(accounting.cost(HAIKU, o), (40 * 0.10 + 600 * 0.125 + 900 * 0.50) / 1e6, places=12)
        r = accounting.normalize_usage("anthropic", dict(u, cache_creation_input_tokens=0, cache_read_input_tokens=600,
                                                         cache_creation={}))
        self.assertEqual((r["input"], r["cached"], r["cache_write"]), (640, 600, 0))
        self.assertAlmostEqual(accounting.cost(HAIKU, r, batch=True), (40 * 0.05 + 600 * 0.005 + 900 * 0.25) / 1e6,
                               places=12)
        h = accounting.normalize_usage("anthropic", dict(u, cache_creation={"ephemeral_5m_input_tokens": 0,
                                                                            "ephemeral_1h_input_tokens": 600}))
        self.assertEqual((h["cache_write"], h["cache_write_1h"]), (0, 600))
        self.assertAlmostEqual(accounting.cost(HAIKU, h), (40 * 0.10 + 600 * 0.20 + 900 * 0.50) / 1e6, places=12)

    def test_haiku_long_context_prices(self):
        small = {"input": 100000, "cached": 0, "cache_write": 0, "output": 0}
        big = {"input": 100001, "cached": 0, "cache_write": 0, "output": 1000}
        self.assertAlmostEqual(accounting.cost(HAIKU, small), 0.01, places=10)
        self.assertAlmostEqual(accounting.cost(HAIKU, big), (100001 * 0.50 + 1000 * 2.50) / 1e6, places=10)
        self.assertAlmostEqual(accounting.cost(HAIKU, big, batch=True), (100001 * 0.25 + 1000 * 1.25) / 1e6, places=10)

    def test_worst_case_bounds_any_capped_call(self):
        for p in (LUNA, GROK, DS, HAIKU):
            for b in (False, True):
                for n in (3000, 150000):
                    full = {"input": n, "cached": 0, "cache_write": 0, "output": 2000}
                    self.assertGreaterEqual(accounting.worst_case(p, n, 2000, batch=b) + 1e-12,
                                            accounting.cost(p, full, batch=b))
                    if p is HAIKU:
                        cw = dict(full, cache_write=n)
                        self.assertGreaterEqual(accounting.worst_case(p, n, 2000, batch=b) + 1e-12,
                                                accounting.cost(p, cw, batch=b))

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

    def test_anthropic_batch_round_trip(self):
        t = providers.MockTransport(echo_model(), batch_polls=3)
        p = providers.AnthropicProvider("claude-haiku-5-5", t, sleep=lambda s: None)
        bid = p.submit_batch([("fib-sley-1", self.MSGS), ("parse_int-c-2", self.MSGS)])
        b = p.wait_batch(bid)
        got = p.batch_results(b)
        self.assertEqual(sorted(got), ["fib-sley-1", "parse_int-c-2"])
        self.assertTrue(all(r.batch and r.finish == "stop" and "return 1" in r.text for r in got.values()))
        urls = [u for _, u in t.calls]
        self.assertIn("https://api.anthropic.com/v1/messages/batches", urls)
        self.assertTrue(any(u.endswith("/results") for u in urls))
        params = t.msg_batches[bid]["_requests"][0]["params"]
        self.assertEqual(params["max_tokens"], 2000)
        self.assertEqual(params["output_config"], {"effort": "low"})
        self.assertEqual(params["system"][0]["cache_control"], {"type": "ephemeral"})
        self.assertEqual([m["role"] for m in params["messages"]], ["user"])
        # the second request read the system prefix the first one wrote
        u = sorted((r.usage for r in got.values()), key=lambda u: u["cache_read_input_tokens"])
        self.assertGreater(u[0]["cache_creation_input_tokens"], 0)
        self.assertGreater(u[1]["cache_read_input_tokens"], 0)

    def test_anthropic_messages_cache_truncation_and_errors(self):
        t = providers.MockTransport(echo_model(finish="length"))
        p = providers.AnthropicProvider("claude-haiku-5-5", t)
        r1, r2 = p.complete(self.MSGS), p.complete(self.MSGS)
        self.assertEqual(r1.finish, "length")
        self.assertFalse(r1.batch)
        self.assertGreater(r1.usage["cache_creation_input_tokens"], 0)
        self.assertGreater(r2.usage["cache_read_input_tokens"], 0)
        # under the 512-token minimum the API caches nothing
        short = [{"role": "system", "content": "x" * 400}, {"role": "user", "content": "hi"}]
        r3 = p.complete(short)
        self.assertEqual((r3.usage["cache_creation_input_tokens"], r3.usage["cache_read_input_tokens"]), (0, 0))
        self.assertEqual(providers.AnthropicProvider.parse(
            {"content": [{"type": "text", "text": "x"}], "stop_reason": "refusal", "usage": {}}, False).finish, "stop")

        class Overloaded:
            def request(self, *a):
                return 529, b'{"type":"error","error":{"type":"overloaded_error"}}'
        with self.assertRaises(providers.InfraError):
            providers.AnthropicProvider("m", Overloaded(), sleep=lambda s: None).complete(self.MSGS)

        class Bad:
            def request(self, *a):
                return 400, b'{"type":"error","error":{"type":"invalid_request_error"}}'
        with self.assertRaises(providers.InfraError):
            providers.AnthropicProvider("m", Bad()).complete(self.MSGS)

    def test_anthropic_batch_errored_items_are_infrastructure(self):
        class T(providers.MockTransport):
            def request(self, method, url, headers, body=None):
                if url.endswith("/results"):
                    return 200, (json.dumps({"custom_id": "a", "result": {"type": "errored", "error": {"type": "x"}}})
                                 + "\n" + json.dumps({"custom_id": "b", "result": {"type": "expired"}}) + "\n").encode()
                return super().request(method, url, headers, body)
        t = T(echo_model())
        p = providers.AnthropicProvider("claude-haiku-5-5", t, sleep=lambda s: None)
        got = p.batch_results(p.wait_batch(p.submit_batch([("a", self.MSGS), ("b", self.MSGS)])))
        self.assertTrue(all(isinstance(v, providers.InfraError) for v in got.values()))

    def test_claude_code_cli_call(self):
        t = providers.MockTransport(echo_model())
        p = providers.make("claude-code-haiku-5-5", PRICES["models"]["claude-code-haiku-5-5"], t, False, 2000)
        self.assertIsInstance(p, providers.ClaudeCodeProvider)
        a = p.argv("SYS")
        self.assertEqual(a[:4], ["claude", "-p", "--output-format", "json"])
        self.assertEqual(a[a.index("--model") + 1], "claude-haiku-5-5")
        self.assertEqual(a[a.index("--tools") + 1], "")
        self.assertEqual(a[a.index("--system-prompt") + 1], "SYS")
        for f in ("--safe-mode", "--strict-mcp-config", "--no-session-persistence", "--disable-slash-commands"):
            self.assertIn(f, a)
        self.assertEqual(p.env(), {"CLAUDE_CODE_MAX_OUTPUT_TOKENS": "2000"})
        r = p.complete(self.MSGS)
        self.assertEqual((r.finish, r.batch), ("stop", False))
        self.assertIn("return 1", r.text)
        self.assertEqual(r.usage["claude_code"]["modelUsage"].keys(), {"claude-haiku-5-5"})
        tok = accounting.normalize_usage("claude-code", r.usage)
        self.assertEqual(tok["input"], r.usage["input_tokens"] + r.usage["cache_read_input_tokens"]
                         + r.usage["cache_creation_input_tokens"])
        c = p.calibrate()
        self.assertGreater(accounting.normalize_usage("claude-code", c.usage)["input"], 20)

    def test_claude_code_retries_carry_the_transcript(self):
        msgs = self.MSGS + [{"role": "assistant", "content": "```python\nx\n```"},
                            {"role": "user", "content": "The visible tests failed."}]
        f = providers.flatten(msgs)
        self.assertIn("<previous_attempt>", f)
        self.assertEqual(providers.unflatten(f), msgs[1:])
        self.assertEqual(providers.flatten(self.MSGS), "hello")

    def test_claude_code_rejects_unpinned_or_failed_calls(self):
        p = providers.ClaudeCodeProvider("claude-haiku-5-5", None, probe=False)
        ok = {"type": "result", "subtype": "success", "is_error": False, "result": "x", "stop_reason": "end_turn",
              "usage": {"input_tokens": 10, "output_tokens": 5}, "modelUsage": {"claude-haiku-5-5": {}}}
        self.assertEqual(p.parse(0, json.dumps(ok).encode(), b"").finish, "stop")
        with self.assertRaises(providers.InfraError):  # a different model answered
            p.parse(0, json.dumps(dict(ok, modelUsage={"claude-opus-5-5": {}})).encode(), b"")
        with self.assertRaises(providers.InfraError):  # an extra model ran alongside
            p.parse(0, json.dumps(dict(ok, modelUsage={"claude-haiku-5-5": {}, "claude-sonnet-5-5": {}})).encode(), b"")
        with self.assertRaises(providers.InfraError):
            p.parse(1, b"", b"Not logged in")
        with self.assertRaises(providers.InfraError):
            p.parse(1, json.dumps(dict(ok, is_error=True, subtype="error_during_execution")).encode(), b"")
        cut = dict(ok, usage={"input_tokens": 10, "output_tokens": 2000})
        self.assertEqual(p.parse(0, json.dumps(cut).encode(), b"").finish, "length")
        self.assertEqual(p.parse(0, json.dumps(dict(ok, stop_reason="max_tokens")).encode(), b"").finish, "length")

    def test_claude_code_probe_drops_flags_the_cli_lacks(self):
        class Old:
            def run(self, argv, stdin, env=None, timeout=None):
                return 0, b"--output-format --system-prompt --tools --model --strict-mcp-config", b""
        p = providers.ClaudeCodeProvider("claude-haiku-5-5", Old())
        self.assertEqual(p.flags, ["--strict-mcp-config"])

        class Ancient:
            def run(self, argv, stdin, env=None, timeout=None):
                return 0, b"--output-format --model", b""
        with self.assertRaises(providers.InfraError):
            providers.ClaudeCodeProvider("claude-haiku-5-5", Ancient())

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
        for var, model, price in (("XAI_API_KEY", "grok-build-0.1", GROK),
                                  ("ANTHROPIC_API_KEY", "claude-haiku-5-5", HAIKU)):
            env = os.environ.pop(var, None)
            try:
                with self.assertRaises(SystemExit):
                    providers.make(model, price, providers.MockTransport(echo_model()), True, 2000)
            finally:
                if env is not None:
                    os.environ[var] = env
        p = providers.make("claude-haiku-5-5", HAIKU, providers.MockTransport(echo_model()), False, 2000)
        self.assertIsInstance(p, providers.AnthropicProvider)
        self.assertEqual(p.model, "claude-haiku-5-5")


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
class Intervals(unittest.TestCase):
    @staticmethod
    def cell(spend, think):
        """task -> counters, from per-task (spend per trial, thinking per call) with 3 trials and 3 calls each."""
        return {t: {"spend": 3 * s, "trials": 3, "success": 3, "think": 3 * k, "calls": 3, "thinking_calls": 3 if k else 0}
                for t, (s, k) in enumerate(zip(spend, think))}

    def test_sign_test(self):
        self.assertAlmostEqual(intervals.sign_test(18, 0), 2 / 2 ** 18)
        self.assertEqual(intervals.sign_test(0, 0), 1.0)
        self.assertEqual(intervals.sign_test(3, 3), 1.0)

    def test_paired_bootstrap_point_values_and_coverage(self):
        py = self.cell([1.0, 2.0, 3.0, 4.0], [0, 100, 0, 100])
        sl = self.cell([1.5, 2.5, 3.5, 4.5], [200, 300, 200, 300])
        c = intervals.compare(sl, py, draws=2000)
        self.assertAlmostEqual(c["usd_per_trial_ratio"], 12 / 10)
        self.assertAlmostEqual(c["usd_per_trial_delta"], 0.5)
        # a constant per-task delta has no spread under paired resampling
        self.assertAlmostEqual(c["usd_per_trial_delta_ci95"][0], 0.5)
        self.assertAlmostEqual(c["usd_per_trial_delta_ci95"][1], 0.5)
        self.assertAlmostEqual(c["thinking_per_call_delta"], 200)
        self.assertEqual((c["tasks_thinking_more"], c["tasks_thinking_less"]), (4, 0))
        lo, hi = c["usd_per_trial_ratio_ci95"]
        self.assertTrue(lo <= c["usd_per_trial_ratio"] <= hi)

    def test_seeded_and_reproducible(self):
        py = self.cell([1, 2, 3, 4, 5], [0, 50, 0, 80, 10])
        sl = self.cell([2, 1, 4, 4, 7], [90, 0, 300, 10, 40])
        self.assertEqual(intervals.compare(sl, py, draws=500), intervals.compare(sl, py, draws=500))

    def test_run_dir_end_to_end(self):
        d = tempfile.mkdtemp()
        rows = []
        for task in ("a", "b"):
            for lang, cost, think in (("python", 1e-4, 0), ("sley", 2e-4, 100)):
                rows.append({"kind": "call", "model": "m", "task": task, "lang": lang, "billed_tokens": {},
                             "usage": str({"output_tokens_details": {"thinking_tokens": think}})})
                rows.append({"kind": "trial", "model": "m", "task": task, "lang": lang, "rep": 1, "infra": False,
                             "success": True, "cost_usd": cost})
        with open(os.path.join(d, "results.jsonl"), "w") as f:
            f.write("\n".join(json.dumps(r) for r in rows))
        out = intervals.intervals(d, draws=100)
        s = out["models"]["m"]["sley"]
        self.assertAlmostEqual(s["vs_baseline"]["usd_per_trial_ratio"], 2.0)
        self.assertEqual(s["share_of_calls_thinking"], 1.0)
        self.assertEqual(out["models"]["m"]["python"]["share_of_calls_thinking"], 0.0)
        self.assertIn("| sley | 2.00x", open(os.path.join(d, "intervals.md")).read())
        shutil.rmtree(d)


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


PY_MASK = """def mask_digits(s: str) -> str:
    total = sum(c.isdigit() for c in s)
    out, seen = [], 0
    for c in s:
        if c in "0123456789":
            seen += 1
            out.append("*" if seen <= total - 4 else c)
        else:
            out.append(c)
    return "".join(out)
"""
PY_CHUNKS = """def chunk_sums(xs: list[int], size: int) -> list[int]:
    if size < 1:
        raise ValueError("size")
    return [sum(xs[i:i + size]) for i in range(0, len(xs), size)]
"""
C_MASK = """#include <string.h>
void mask_digits(const char *s, char *out) {
    size_t total = 0, seen = 0, n = strlen(s);
    for (size_t i = 0; i < n; i++) total += s[i] >= '0' && s[i] <= '9';
    for (size_t i = 0; i < n; i++) {
        int d = s[i] >= '0' && s[i] <= '9';
        seen += d;
        out[i] = d && total > 4 && seen <= total - 4 ? '*' : s[i];
    }
    out[n] = 0;
}
"""
C_CHUNKS = """#include <stddef.h>
int chunk_sums(const long long *xs, size_t xs_len, long long size, long long *out, size_t *out_len) {
    if (size < 1) return 1;
    *out_len = 0;
    for (size_t i = 0; i < xs_len; i += (size_t)size) {
        long long t = 0;
        for (size_t j = i; j < xs_len && j < i + (size_t)size; j++) t += xs[j];
        out[(*out_len)++] = t;
    }
    return 0;
}
"""


class HarnessV2(unittest.TestCase):
    def test_signatures(self):
        self.assertEqual(langs.signature("mask_digits", "python"), "def mask_digits(s: str) -> str")
        self.assertTrue(langs.signature("chunk_sums", "c").startswith(
            "int chunk_sums(const long long *xs, size_t xs_len, long long size, long long *out, size_t *out_len)"))
        self.assertTrue(langs.signature("mask_digits", "c").startswith("void mask_digits(const char *s, char *out)"))
        self.assertEqual(langs.signature("run_length_decode", "rust"), "pub fn run_length_decode(s: &str) -> Result<String, String>")
        self.assertEqual(langs.signature("dense_rank", "go"), "func DenseRank(xs []int64) []int64  (package solution)")
        self.assertEqual(langs.signature("to_roman", "ts"), "export function toRoman(n: number): string  (throw an Error on error)")
        self.assertEqual(langs.lit("c", 'a"\\b'), '"a\\"\\\\b"')
        self.assertEqual(run.task_prompt("to_roman", "python").splitlines()[-1], "  to_roman(4000) -> error")

    def score(self, task, lang, src):
        d = tempfile.mkdtemp()
        try:
            ok, out = langs.check(task, lang, src, os.path.join(d, "v"))
            return ok, langs.oracle(task, lang, src, os.path.join(d, "o")), out
        finally:
            shutil.rmtree(d)

    def test_python_string_and_list_results(self):
        for task, src in (("mask_digits", PY_MASK), ("chunk_sums", PY_CHUNKS)):
            ok, o, out = self.score(task, "python", src)
            self.assertTrue(ok, out)
            self.assertEqual(o["passed"], 128, o)
        ok, o, _ = self.score("chunk_sums", "python", PY_CHUNKS.replace("return [", "return tuple(").replace(" size)]", " size))"))
        self.assertFalse(ok)
        self.assertEqual(o["first_failure"]["got"][:4], "BAD ")

    @unittest.skipUnless(shutil.which("gcc"), "gcc")
    def test_c_string_and_list_results(self):
        for task, src in (("mask_digits", C_MASK), ("chunk_sums", C_CHUNKS)):
            ok, o, out = self.score(task, "c", src)
            self.assertTrue(ok, out)
            self.assertEqual(o["passed"], 128, o)
        ok, o, _ = self.score("mask_digits", "c", C_MASK.replace("'*'", "'#'"))
        self.assertFalse(ok)
        self.assertLess(o["passed"], 128)

    def test_task_set_flag(self):
        d = tempfile.mkdtemp()
        try:
            r = subprocess.run([sys.executable, os.path.join(HERE, "run.py"), "--out", d, "--task-set", "v2",
                                "--tasks", "fib"], capture_output=True, text=True, timeout=60)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn("not in task set v2: fib", r.stderr)
            r = subprocess.run([sys.executable, os.path.join(HERE, "run.py"), "--task-set", "v2", "--tier", "full",
                                "--project", "--models", "claude-haiku-5-5"], capture_output=True, text=True, timeout=60)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(json.loads(r.stdout)["trials_per_model"] % 24, 0)
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
            claude_first = [x for x in rows if x["kind"] == "call" and x["model"] == "claude-haiku-5-5" and x["attempt"] == 1]
            self.assertTrue(claude_first and all(x["batch"] for x in claude_first))
            # billed as the Messages API reports it: cache reads and writes beside input_tokens
            self.assertTrue(all("cache_read_input_tokens" in x["usage"] and "cache_write_1h" in x["billed_tokens"]
                                for x in claude_first))
            grok = [x for x in rows if x["kind"] == "call" and x["model"] == "grok-build-0.1"]
            self.assertTrue(grok and not any(x["batch"] for x in grok))
            self.assertTrue(all(x["attempt"] <= 3 for x in rows if x["kind"] == "call"))
            import summarize
            s = summarize.summarize(d)
            self.assertEqual(s["mode"], "mock")
        finally:
            shutil.rmtree(d)

    def test_claude_code_mock_run(self):
        d, r = self.run_cli("--models", "claude-code-haiku-5-5")
        try:
            self.assertEqual(r.returncode, 0, r.stderr)
            rows = [json.loads(l) for l in open(os.path.join(d, "results.jsonl"))]
            self.assertEqual(sum(1 for x in rows if x["kind"] == "trial"), 6)
            self.assertEqual(sum(1 for x in rows if x["kind"] == "calibration"), 1)
            calls = [x for x in rows if x["kind"] == "call"]
            self.assertTrue(calls and not any(x["batch"] for x in calls))
            self.assertTrue(any(x["attempt"] > 1 for x in calls))  # retries go through the flattened transcript
            import summarize
            s = summarize.summarize(d)
            md = open(os.path.join(d, "summary.md")).read()
            self.assertIn("Claude Code cloud session: tokens as reported; $ estimated at API list price, not billed per token", md)
            self.assertIn("Claude Code overhead", md)
            self.assertIn("calibration", s)
            # 1-hour cache writes count toward the cache writes the overhead line reports
            cal = os.path.join(d, "calibration.json")
            c = json.load(open(cal))
            c["billed_tokens"].update(cache_write=0, cache_write_1h=1327)
            json.dump(c, open(cal, "w"))
            summarize.summarize(d)
            self.assertIn("1327 cache writes", open(os.path.join(d, "summary.md")).read())
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
        small = run.projection(PRICES, ["claude-haiku-5-5", "grok-build-0.1"], "small", 1)
        full = run.projection(PRICES, ["claude-haiku-5-5", "grok-build-0.1"], "full", 3)
        self.assertEqual(small["trials_per_model"], 90)
        self.assertEqual(full["trials_per_model"], 324)
        self.assertLess(small["expected"], 0.5)
        self.assertLess(full["expected"], 2.0)
        self.assertLess(small["ceiling"], 10.0)  # the full tier's theoretical ceiling passes the guard; the guard binds
        haiku = full["rows"][0]
        # per trial: first attempt on the Batch API + 0.3 standard-rate retries
        self.assertAlmostEqual(haiku["per_attempt_first"], (1300 * 0.05 + 88 * 0.0625 + 1560 * 0.25) / 1e6, places=10)
        self.assertAlmostEqual(haiku["per_attempt_retry"], (1300 * 0.10 + 88 * 0.01 + 1560 * 0.50) / 1e6, places=10)


if __name__ == "__main__":
    unittest.main()
