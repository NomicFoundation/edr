#!/usr/bin/env python3
"""Regenerate the solx compiler-output fixtures from their inputs.

Usage: generate_solx_fixtures.py <path-to-solx-binary> [fixture ...]

Splices each fixture's source files (from `sources/`) into its committed
input JSON — whose `content` fields are deliberately empty — runs
`solx --standard-json`, and rewrites the output JSON. Run after bumping the
solx version or editing a fixture source, then re-run the consuming tests
(`cargo test -p edr_provider --features test-utils solx_stack_trace`).

solx release binaries: https://github.com/NomicFoundation/solx/releases
(or the mirror at https://solx-releases-mirror.hardhat.org).

The `scenarios` fixture is NOT regenerable by this script: its input also
depends on forge-std sources whose contents are scrubbed from the committed
JSON. It was generated from a hardhat project with `@nomicfoundation/hardhat-solx`
configured; regenerate it there and re-scrub the non-fixture `content` fields.
"""

import json
import subprocess
import sys
from pathlib import Path

FIXTURES_DIR = Path(__file__).parent

# fixture name -> (input json, {source name in input: file under sources/}, output json)
FIXTURES = {
    "counter": (
        "solx_compiler_input.json",
        {"Counter.sol": "Counter.sol"},
        "solx_compiler_output.json",
    ),
    "long_tail": (
        "solx_compiler_input_long_tail.json",
        {"project/contracts/LongTail.sol": "LongTail.sol"},
        "solx_compiler_output_long_tail.json",
    ),
}


def generate(solx: str, name: str) -> None:
    input_path, source_map, output_path = FIXTURES[name]
    compiler_input = json.loads((FIXTURES_DIR / input_path).read_text())

    for source_name, source_file in source_map.items():
        compiler_input["sources"][source_name]["content"] = (
            FIXTURES_DIR / "sources" / source_file
        ).read_text()
    scrubbed = [
        source_name
        for source_name, source in compiler_input["sources"].items()
        if source["content"] == ""
    ]
    if scrubbed:
        raise SystemExit(f"{name}: sources without content, cannot compile: {scrubbed}")

    result = subprocess.run(
        [solx, "--standard-json"],
        input=json.dumps(compiler_input),
        capture_output=True,
        text=True,
        check=True,
    )
    output = json.loads(result.stdout)

    errors = [e for e in output.get("errors", []) if e.get("severity") == "error"]
    if errors:
        raise SystemExit(
            f"{name}: solx reported errors:\n"
            + "\n".join(e.get("formattedMessage", str(e)) for e in errors)
        )

    (FIXTURES_DIR / output_path).write_text(json.dumps(output, indent=1) + "\n")
    print(f"{name}: wrote {output_path}")


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    solx = sys.argv[1]
    version = subprocess.run(
        [solx, "--version"], capture_output=True, text=True, check=True
    ).stdout.strip()
    print(f"using: {version}")
    for name in sys.argv[2:] or FIXTURES:
        generate(solx, name)


if __name__ == "__main__":
    main()
