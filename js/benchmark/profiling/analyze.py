#!/usr/bin/env python3
"""
Component attribution for a `perf script` capture of a Hardhat 3 solidity-test run.

The thing that actually separates the components in this workload is the
process/thread role, not the DSO: EDR's native runner lives on `tokio-rt-worker`
threads, Hardhat's JS on the main thread, artifact/build-info file I/O on
`libuv-worker`, and `vm.ffi` shells out to whole `npm`/`node` subprocesses.
So `components` groups by comm first, and `frames` drills into one role.

Subcommands
-----------
  components <stacks>            CPU share per process/thread role
  frames     <stacks> [--thread] hottest leaf frames within a role (demangled)
  pattern    <stacks> [--pat]    inclusive share of samples whose stack contains
                                 a symbol substring -- used to size specific code
                                 paths, e.g. native artifact linking
  rate       <stacks>            achieved sample rate + implied thread concurrency,
                                 to confirm perf was not throttled

Requires `rustfilt` on PATH for Rust v0 demangling (cargo install rustfilt);
degrades to mangled names without it.
"""
import argparse
import re
import subprocess
import sys
from collections import Counter

FRAME = re.compile(r"^\s+[0-9a-f]+ (.*?)(?: \((.*)\))?$")
HEADER_TS = re.compile(r"\s(\d+\.\d+):\s")

# comm -> component role. Anything unlisted is reported verbatim so a new thread
# name shows up as itself rather than being silently bucketed.
ROLE = {
    "node-prof": "main-process (JS/hardhat + napi calls)",
    "node": "FFI subprocess (node)",
    "npm": "FFI subprocess (npm)",
    "sh": "FFI subprocess (sh)",
    "tokio-rt-worker": "EDR solidity-test runner (native)",
    "libuv-worker": "node threadpool (file I/O)",
}

# Mangled-name substrings for code paths worth sizing individually. Rust v0
# mangling embeds `<len><name>`, hence the digit prefixes.
DEFAULT_PATTERNS = {
    "napi LinkingOutput::link (artifact deser + linking)": [
        "13LinkingOutput",
        "6linker6Linker",
        "get_linked_artifacts",
    ],
    "serde_json (JSON deser in native code)": ["10serde_json"],
    "napi string/JS value conversion": ["9js_values"],
    "revm interpreter step": ["16revm_interpreter"],
    "foundry cheatcodes": ["18foundry_cheatcodes"],
}


def role_of(comm):
    return ROLE.get(comm, f"other ({comm})")


def demangle(names):
    """Batch-demangle via rustfilt; identity mapping if unavailable."""
    names = list(names)
    if not names:
        return {}
    try:
        p = subprocess.run(
            ["rustfilt"], input="\n".join(names), capture_output=True, text=True
        )
        out = p.stdout.splitlines()
        if len(out) == len(names):
            return dict(zip(names, out))
    except FileNotFoundError:
        print("warning: rustfilt not found; symbols stay mangled", file=sys.stderr)
    return {n: n for n in names}


def shorten(sym):
    """Collapse the enormous generic argument lists revm/foundry monomorphise into."""
    return re.sub(r"<(.{40,})>", "<...>", sym)


def parse(path):
    """Yield (comm, [(symbol, dso), ...]) per sample, leaf frame first."""
    stacks = []
    cur = None
    comm = None
    with open(path, errors="replace") as fh:
        for line in fh:
            if not line.strip():
                if cur is not None:
                    stacks.append((comm, cur))
                cur = None
                continue
            if not line.startswith(("\t", " ")):
                comm = line.split()[0]
                cur = []
                continue
            if cur is None:
                continue
            m = FRAME.match(line.rstrip("\n"))
            if m:
                cur.append((m.group(1), m.group(2)))
    if cur is not None:
        stacks.append((comm, cur))
    return stacks


def cmd_components(args):
    stacks = parse(args.stacks)
    total = len(stacks)
    hz = args.hz
    print(f"total samples: {total}  ({hz} Hz => ~{total/hz:.2f} CPU-seconds)\n")
    roles = Counter(role_of(c) for c, _ in stacks)
    print("=== CPU by component (process/thread role) ===")
    for r, n in roles.most_common():
        print(f"  {r:42s} {n:7d}  {100.0*n/total:6.2f}%  {n/hz:6.2f}s")


def cmd_frames(args):
    stacks = parse(args.stacks)
    sel = [st for c, st in stacks if args.thread is None or c == args.thread]
    leaf = Counter()
    resolved = 0
    for st in sel:
        for s, _ in st:
            if s != "[unknown]":
                leaf[s.split("+")[0]] += 1
                resolved += 1
                break
    if not resolved:
        print(f"no resolved frames for thread={args.thread}", file=sys.stderr)
        return
    names = [s for s, _ in leaf.most_common(args.top)]
    dm = demangle(names)
    scope = args.thread or "all threads"
    print(f"=== hottest leaf frames: {scope} ({resolved} resolved samples) ===")
    for s, n in leaf.most_common(args.top):
        print(f"  {n:6d}  {100.0*n/resolved:6.2f}%  {shorten(dm.get(s, s))[:160]}")


def cmd_pattern(args):
    stacks = parse(args.stacks)
    total = len(stacks)
    pats = (
        {args.pat: [args.pat]} if args.pat else DEFAULT_PATTERNS
    )
    print(f"=== inclusive share of samples containing pattern ({total} samples) ===")
    for label, subs in pats.items():
        n = sum(
            1
            for _, st in stacks
            if any(any(sub in f for sub in subs) for f, _ in st)
        )
        print(f"  {label:52s} {n:7d}  {100.0*n/total:7.3f}%  {n/args.hz:.3f}s")


def cmd_rate(args):
    """Confirm perf actually sampled at the requested rate (no kernel throttling)."""
    ts = []
    with open(args.stacks, errors="replace") as fh:
        for line in fh:
            if line.strip() and not line.startswith(("\t", " ")):
                m = HEADER_TS.search(line)
                if m:
                    ts.append(float(m.group(1)))
    if not ts:
        print("no sample timestamps parsed", file=sys.stderr)
        return
    span = max(ts) - min(ts)
    per_wall = len(ts) / span if span else float("nan")
    print(f"samples          : {len(ts)}")
    print(f"wall span        : {span:.3f}s")
    print(f"samples/wall-sec : {per_wall:.1f}")
    print(f"requested rate   : {args.hz} Hz per thread")
    print(f"=> implies ~{per_wall/args.hz:.2f} threads on-CPU on average")
    print(
        "\nIf samples/wall-sec is not ~= (threads x requested rate), the kernel "
        "throttled;\ncross-check with: sudo perf report -i <data> --stats | grep -iE 'THROTTLE|LOST'"
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--hz", type=float, default=999.0, help="sample rate used when recording")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("components")
    p.add_argument("stacks")
    p.set_defaults(fn=cmd_components)

    p = sub.add_parser("frames")
    p.add_argument("stacks")
    p.add_argument("--thread", default=None, help="comm to drill into, e.g. tokio-rt-worker")
    p.add_argument("--top", type=int, default=15)
    p.set_defaults(fn=cmd_frames)

    p = sub.add_parser("pattern")
    p.add_argument("stacks")
    p.add_argument("--pat", default=None, help="single symbol substring instead of the defaults")
    p.set_defaults(fn=cmd_pattern)

    p = sub.add_parser("rate")
    p.add_argument("stacks")
    p.set_defaults(fn=cmd_rate)

    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
