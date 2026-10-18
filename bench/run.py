#!/usr/bin/env python3
"""Reproducible benchmarks for Blaze. Writes bench/results.json.

usage: bench/run.py <rust-corpus-dir> <crate-with-tests-dir> [sample-size]
Needs: release build, rustfmt, cargo-nextest, cargo-deny. Each number is best-of-3 wall time.
"""
import json, os, random, shutil, subprocess, sys, time, platform

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(HERE, "..", "target", "release")
corpus, crate = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
N = int(sys.argv[3]) if len(sys.argv) > 3 else 2000


def timed(cmd, cwd, cold=False, n=3, shell=False):
    best = 1e9
    for _ in range(n):
        if cold:
            shutil.rmtree(os.path.join(cwd, ".blaze_cache"), ignore_errors=True)
        s = time.perf_counter()
        subprocess.run(cmd, cwd=cwd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, shell=shell)
        best = min(best, time.perf_counter() - s)
    return round(best, 4)


files = []
for d, _, fs in os.walk(corpus):
    files += [os.path.join(d, f) for f in fs if f.endswith(".rs")]
files.sort()
random.seed(1)
sample = random.sample(files, min(N, len(files)))
listfile = os.path.join(HERE, ".sample.txt")
open(listfile, "w").write("\n".join(sample))

r = {"machine": f"{platform.machine()} {platform.system()}, {os.cpu_count()} cores", "files": len(sample)}
forge = [os.path.join(BIN, "forge"), "--check", *sample]
r["fmt_rustfmt"] = timed(f"xargs -n 40 -P 8 rustfmt --edition 2021 --check < {listfile}", corpus, shell=True)
r["fmt_forge_cold"] = timed(forge, corpus, cold=True)
subprocess.run(forge, cwd=corpus, capture_output=True)
r["fmt_forge_warm"] = timed(forge, corpus)
r["lint_ember"] = timed([os.path.join(BIN, "ember"), *sample], corpus)
r["lint_ember_full_corpus"] = timed([os.path.join(BIN, "ember"), "."], corpus)
r["full_corpus_files"] = len(files)
r["full_corpus_mb"] = round(sum(os.path.getsize(f) for f in files) / 1e6)

subprocess.run(["cargo", "test", "--no-run", "-q"], cwd=crate, capture_output=True)
r["test_cargo_test"] = timed(["cargo", "test", "-q"], crate)
r["test_nextest"] = timed(["cargo", "nextest", "run"], crate)
r["test_spark"] = timed([os.path.join(BIN, "spark")], crate)
subprocess.run([os.path.join(BIN, "spark"), "--cached"], cwd=crate, capture_output=True)
r["test_spark_cached"] = timed([os.path.join(BIN, "spark"), "--cached"], crate)

shutil.rmtree(os.path.join(crate, "target"), ignore_errors=True)
r["clippy_cold"] = timed(["cargo", "clippy", "-q"], crate, n=1)
r["clippy_warm"] = timed(["cargo", "clippy", "-q"], crate)
r["lint_ember_crate"] = timed([os.path.join(BIN, "ember"), "."], crate)
subprocess.run(["cargo", "test", "--no-run", "-q"], cwd=crate, capture_output=True)

deny = os.path.join(HERE, "deny.toml")
r["deny_cargo_deny"] = timed(["cargo", "deny", "--config", deny, "check", "licenses", "bans", "sources"], os.path.join(HERE, ".."))
subprocess.run([os.path.join(BIN, "ward")], cwd=os.path.join(HERE, ".."), capture_output=True)
r["deny_ward_warm"] = timed([os.path.join(BIN, "ward")], os.path.join(HERE, ".."))
json.dump(r, open(os.path.join(HERE, "results.json"), "w"), indent=2)
print(json.dumps(r, indent=2))
