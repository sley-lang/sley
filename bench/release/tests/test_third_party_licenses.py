"""Offline tests of the third-party license texts shipped in the release archive (contract section 17)."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from bench.release.tests.test_packaging import packaging

ROOT = Path(__file__).resolve().parents[3]
third_party = packaging.third_party

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def fake_workspace(root: Path, crates: dict[str, dict[str, str]]) -> dict:
    """A Cargo.lock and cargo-metadata-shaped document over fake crate sources."""
    lock = ["version = 4", ""]
    packages = [{"name": "sley-cli", "version": "9.9.9", "source": None, "manifest_path": str(root / "Cargo.toml")}]
    for index, (name, files) in enumerate(sorted(crates.items())):
        directory = root / "registry" / f"{name}-1.0.{index}"
        directory.mkdir(parents=True)
        (directory / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
        (directory / "src.rs").write_text("// not a license\n", encoding="utf-8")
        for file_name, text in files.items():
            (directory / file_name).write_text(text, encoding="utf-8")
        lock += ["[[package]]", f'name = "{name}"', f'version = "1.0.{index}"', f'source = "{REGISTRY}"', f'checksum = "{index:064x}"', ""]
        packages.append({
            "name": name,
            "version": f"1.0.{index}",
            "source": REGISTRY,
            "license": "BSD-3-Clause",
            "license_file": None,
            "repository": f"https://example.invalid/{name}",
            "authors": [f"{name} authors"],
            "manifest_path": str(directory / "Cargo.toml"),
        })
    lock += ["[[package]]", 'name = "sley-cli"', 'version = "9.9.9"', ""]
    (root / "Cargo.lock").write_text("\n".join(lock), encoding="utf-8")
    # cargo metadata lists packages in no promised order.
    return {"packages": list(reversed(packages))}


class ThirdPartyLicenseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_rendering_is_sorted_verbatim_and_free_of_local_paths(self) -> None:
        metadata = fake_workspace(self.root, {
            "zeta": {"LICENSE-MIT": "Copyright (c) 2020 Zeta Authors\r\nMIT terms  \r\n\r\n", "README.md": "not shipped"},
            "alpha": {"LICENSE": "Copyright (c) 2016 Alpha Authors\nBSD terms\n", "NOTICE": "Alpha notice\n", "AUTHORS": "A. Author\n"},
        })
        first = third_party.render(third_party.collect(self.root, metadata))
        second = third_party.render(third_party.collect(self.root, dict(metadata, packages=list(reversed(metadata["packages"])))))
        self.assertEqual(first, second)
        text = first.decode("utf-8")
        self.assertNotIn(str(self.root), text)
        self.assertNotIn("registry/", text)
        self.assertNotIn("\r", text)
        self.assertFalse([line for line in text.split("\n") if line != line.rstrip()], "no trailing whitespace")
        self.assertNotIn("not shipped", text)
        self.assertNotIn("not a license", text)
        self.assertIn("Copyright (c) 2016 Alpha Authors\nBSD terms\n", text)
        self.assertIn("Copyright (c) 2020 Zeta Authors\nMIT terms\n", text)
        self.assertIn("Alpha notice", text)
        self.assertIn("Checksum (sha256): " + "0" * 64, text)
        self.assertLess(text.index("File: AUTHORS"), text.index("File: LICENSE"))
        self.assertLess(text.index("File: LICENSE\n"), text.index("File: NOTICE"))
        self.assertLess(text.index("\nalpha 1.0.0\n"), text.index("\nzeta 1.0.1\n"))
        self.assertEqual(third_party.indexed_packages(first), [("alpha", "1.0.0"), ("zeta", "1.0.1")])
        self.assertTrue(first.endswith(b"\n") and not first.endswith(b"\n\n"))

    def test_a_crate_without_a_license_file_stops_the_generator(self) -> None:
        metadata = fake_workspace(self.root, {"bare": {"README.md": "no license here"}})
        with self.assertRaises(third_party.LicenseError) as error:
            third_party.collect(self.root, metadata)
        self.assertIn("ships no license file", str(error.exception))

    def test_metadata_and_lock_must_name_the_same_packages(self) -> None:
        metadata = fake_workspace(self.root, {"alpha": {"LICENSE": "a\n"}, "beta": {"LICENSE": "b\n"}})
        dropped = dict(metadata, packages=[p for p in metadata["packages"] if p["name"] != "beta"])
        with self.assertRaises(third_party.LicenseError):
            third_party.collect(self.root, dropped)
        renamed = dict(metadata, packages=[dict(p, version="2.0.0") if p["name"] == "beta" else p for p in metadata["packages"]])
        with self.assertRaises(third_party.LicenseError):
            third_party.collect(self.root, renamed)

    def test_package_index_drift_is_named(self) -> None:
        metadata = fake_workspace(self.root, {"alpha": {"LICENSE": "a\n"}, "beta": {"LICENSE": "b\n"}})
        rendered = third_party.render(third_party.collect(self.root, metadata))
        self.assertEqual(third_party.package_problems(self.root, rendered), [])
        stale = rendered.replace(b"  beta 1.0.1\n", b"  beta 0.9.0\n")
        self.assertEqual(
            third_party.package_problems(self.root, stale),
            ["missing:beta 1.0.1", "unlocked:beta 0.9.0"],
        )
        self.assertEqual(third_party.package_problems(self.root, b"no index\n"), ["index:package index missing"])

    def test_the_tracked_file_names_the_locked_and_inventoried_crates(self) -> None:
        data = (ROOT / third_party.OUTPUT_NAME).read_bytes()
        self.assertEqual(third_party.package_problems(ROOT, data), [])
        inventory = json.loads(packaging.INVENTORY.read_text(encoding="utf-8"))
        inventoried = sorted(
            (package["name"], package["version"])
            for package in inventory["packages"]
            if package["ecosystem"] == "cargo" and not package["workspace"]
        )
        self.assertEqual(third_party.indexed_packages(data), inventoried)
        # The redistribution obligation that motivated the file: the BSD
        # crates' license texts carry their copyright lines.
        text = data.decode("utf-8")
        for name in ("arrayref", "curve25519-dalek", "ed25519-dalek", "subtle"):
            section = text.split(f"\n{name} ", 1)[1].split("\n" + third_party.RULE, 1)[0]
            self.assertIn("Copyright", section, name)
        self.assertNotIn("/home/", text)

    def test_the_file_is_an_owned_member_and_an_artifact_input(self) -> None:
        self.assertIn(third_party.OUTPUT_NAME, packaging.FIXED_ARTIFACT_MEMBERS)
        self.assertNotIn(third_party.OUTPUT_NAME, packaging.EXECUTABLE_MEMBERS)
        self.assertIn(third_party.OUTPUT_NAME, packaging.ARTIFACT_INPUT_PATHS)
        self.assertIn(third_party.GENERATOR, packaging.ARTIFACT_INPUT_PATHS)
        self.assertTrue((ROOT / third_party.GENERATOR).is_file())

    def stage(self, stage: Path) -> dict:
        binary = self.root / "sley-fixture"
        binary.write_bytes(b"fixture binary")
        return packaging.stage_artifact(binary, stage, commit="a" * 40,
            toolchain={"cargo": "fixture", "rustc": "fixture"},
            working_tree_clean=True, blockers=[])

    def test_staging_ships_the_file_and_records_it_in_licenses_json(self) -> None:
        stage = self.root / "stage"
        self.stage(stage)
        tracked = (ROOT / third_party.OUTPUT_NAME).read_bytes()
        self.assertEqual((stage / third_party.OUTPUT_NAME).read_bytes(), tracked)
        licenses = json.loads((stage / "LICENSES.json").read_text(encoding="utf-8"))
        self.assertEqual(licenses["third_party_licenses"], {
            "file": third_party.OUTPUT_NAME,
            "sha256": packaging.sha256_bytes(tracked),
            "packages": len(third_party.indexed_packages(tracked)),
        })

    def test_staging_refuses_a_file_that_names_other_crates(self) -> None:
        tracked = (ROOT / third_party.OUTPUT_NAME).read_bytes()
        for label, data in (
            ("stale-version", tracked.replace(b"  subtle 2.6.1\n", b"  subtle 2.6.0\n", 1)),
            ("no-index", b"license text without an index\n"),
        ):
            substitute = self.root / f"{label}.txt"
            substitute.write_bytes(data)
            with self.subTest(label=label), patch.object(packaging, "THIRD_PARTY_LICENSES", substitute):
                with self.assertRaises(packaging.PackageError) as error:
                    self.stage(self.root / label)
                self.assertEqual(int(error.exception.code), 72007)

    def test_the_build_refuses_drift_and_a_failed_fetch(self) -> None:
        metadata = fake_workspace(self.root, {"alpha": {"LICENSE": "a\n"}})
        packages = third_party.collect(self.root, metadata)
        ok = subprocess.CompletedProcess(["cargo"], 0, "", "")
        tracked = self.root / "THIRD_PARTY_LICENSES"
        with patch.object(packaging, "THIRD_PARTY_LICENSES", tracked), \
                patch.object(packaging.third_party, "collect", return_value=packages), \
                patch.object(packaging, "run", return_value=ok) as run:
            tracked.write_bytes(third_party.render(packages))
            packaging.verify_third_party_licenses(60)
            self.assertEqual(run.call_args.args[0], ["cargo", "fetch", "--locked"])
            tracked.write_bytes(third_party.render(packages) + b"edited by hand\n")
            with self.assertRaises(packaging.PackageError) as error:
                packaging.verify_third_party_licenses(60)
            self.assertEqual(int(error.exception.code), 72007)
            run.return_value = subprocess.CompletedProcess(["cargo"], 101, "", "offline")
            with self.assertRaises(packaging.PackageError) as error:
                packaging.verify_third_party_licenses(60)
            self.assertEqual(int(error.exception.code), 72000)


if __name__ == "__main__":
    unittest.main()
