import importlib.util
import unittest
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "cargo_audit_gate", Path(__file__).with_name("cargo-audit-gate.py")
)
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def audit_for(identifier, package="rsa", category="vulnerabilities"):
    finding = {
        "package": {"name": package, "version": "0.9.10"},
        "advisory": {"id": identifier},
    }
    result = {"vulnerabilities": {"list": []}, "warnings": {}}
    if category == "vulnerabilities":
        result["vulnerabilities"]["list"].append(finding)
    else:
        result["warnings"][category] = [finding]
    return result


class CargoAuditGateTests(unittest.TestCase):
    def test_known_unused_advisory_is_visible(self):
        failures, unused = gate.assess(audit_for("RUSTSEC-2023-0071"), set())
        self.assertEqual(failures, [])
        self.assertEqual(len(unused), 1)

    def test_enabling_previously_unused_dependency_blocks_publication(self):
        failures, _ = gate.assess(
            audit_for("RUSTSEC-2023-0071"), {("rsa", "0.9.10")}
        )
        self.assertEqual(len(failures), 1)

    def test_new_advisory_blocks_even_when_package_is_unused(self):
        failures, _ = gate.assess(audit_for("RUSTSEC-2099-9999"), set())
        self.assertEqual(len(failures), 1)

    def test_unsound_and_unmaintained_advisories_block(self):
        for category in ("unsound", "unmaintained"):
            with self.subTest(category=category):
                failures, _ = gate.assess(
                    audit_for("RUSTSEC-2099-9999", "lru", category), set()
                )
                self.assertEqual(len(failures), 1)

    def test_graph_parser_handles_duplicate_paths(self):
        tree = "app v1.0.0 (/workspace)\nopenssl v0.10.80 (*)\nopenssl v0.10.80\n"
        self.assertEqual(gate.reachable_packages(tree), {("app", "1.0.0"), ("openssl", "0.10.80")})

    def test_empty_or_malformed_graph_fails_closed(self):
        for tree in ("", "resolver failed\n"):
            with self.subTest(tree=tree):
                with self.assertRaises(ValueError):
                    gate.reachable_packages(tree)


if __name__ == "__main__":
    unittest.main()
