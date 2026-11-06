#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
from pathlib import Path


UNUSED_ADVISORIES = {
    "RUSTSEC-2023-0071": "rsa",
}

# Reachable, but only at build time or with no fix upstream. Each stays printed on
# every run; docs/plans/owner.md O36 holds the review. A new advisory still blocks.
ACCEPTED_ADVISORIES = {
    # utoipa-swagger-ui build script unpacks its own pinned asset, never user input.
    "RUSTSEC-2026-0429": "zip",
    "RUSTSEC-2026-0367": "zip",
    "RUSTSEC-2026-0368": "zip",
    "RUSTSEC-2026-0369": "zip",
    # Unmaintained notice, pulled in by aws-smithy-http. No newer release to take.
    "RUSTSEC-2026-0441": "pin-utils",
}


def reachable_packages(tree):
    import re

    packages = set()
    for line in tree.splitlines():
        if not line.strip():
            continue
        match = re.match(r"^([A-Za-z0-9_-]+) v([^\s]+)(?: |$)", line)
        if not match:
            raise ValueError("Unexpected cargo tree output")
        packages.add(match.groups())
    if not packages:
        raise ValueError("Empty dependency graph")
    return packages


def assess(audit, reachable):
    failures = []
    unused = []
    findings = list(audit["vulnerabilities"]["list"])
    for category in ("unsound", "unmaintained", "yanked"):
        findings.extend(audit.get("warnings", {}).get(category, []))
    for finding in findings:
        package = finding["package"]
        advisory = finding.get("advisory", {})
        identifier = advisory.get("id", "yanked")
        item = f"{identifier}: {package['name']} {package['version']}"
        if (
            UNUSED_ADVISORIES.get(identifier) == package["name"]
            and (package["name"], package["version"]) not in reachable
        ):
            unused.append(item)
        elif ACCEPTED_ADVISORIES.get(identifier) == package["name"]:
            unused.append(f"ACCEPTED {item}")
        else:
            failures.append(item)
    return failures, unused


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", default="aarch64-unknown-linux-gnu")
    arguments = parser.parse_args()
    workspace = Path(__file__).resolve().parents[2] / "rust"
    tree = subprocess.run(
        [
            "cargo", "tree", "--locked", "--workspace",
            "--target", arguments.target,
            "--edges", "normal,build,dev", "--prefix", "none", "--format", "{p}",
            "--features", "ruxlog/vendored-openssl",
        ],
        cwd=workspace, check=True, capture_output=True, text=True,
    )
    audit = subprocess.run(
        ["cargo", "audit", "--json"], cwd=workspace,
        capture_output=True, text=True,
    )
    if audit.returncode not in (0, 1):
        raise RuntimeError("cargo audit could not complete")
    result = json.loads(audit.stdout)
    if "error" in result:
        raise RuntimeError("cargo audit reported an operational error")
    failures, unused = assess(result, reachable_packages(tree.stdout))
    for item in unused:
        print(f"NOT BLOCKING (unused on {arguments.target}, or accepted): {item}")
    for item in failures:
        print(f"BLOCKED: {item}", file=sys.stderr)
    if failures:
        return 1
    print("Dependency gate passed; findings above remain visible.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (KeyError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Dependency gate failed: {error}", file=sys.stderr)
        sys.exit(1)
