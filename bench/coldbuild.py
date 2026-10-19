#!/usr/bin/env python3
"""Cold build of a second project that shares its dependencies with the first one.

usage: bench/coldbuild.py [repeats]   (updates bench/results.json in place)
Each repeat builds fresh projects a, b, c: cargo builds a and b in their own target directories;
blaze builds a and then c into one shared target directory. The minimum over repeats is kept.
"""
import json, os, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
BLAZE = os.path.join(HERE, "..", "target", "release", "blaze")


def make_project(base, name):
    d = os.path.join(base, name)
    subprocess.run(["cargo", "new", "--vcs", "none", "-q", d], capture_output=True)
    open(os.path.join(d, "Cargo.toml"), "a").write("\n[workspace]\n")
    subprocess.run([BLAZE, "add", "serde", "-F", "derive"], cwd=d, capture_output=True)
    subprocess.run([BLAZE, "add", "anyhow", "itoa", "rayon", "clap", "regex"], cwd=d, capture_output=True)
    return d


def timed(cmd, cwd, env=None):
    t0 = time.perf_counter()
    subprocess.run(cmd, cwd=cwd, capture_output=True, env={**os.environ, **(env or {})})
    return round(time.perf_counter() - t0, 2)


def measure(repeats=2):
    best = {}
    for _ in range(repeats):
        base = tempfile.mkdtemp()
        a, b, c = (make_project(base, n) for n in "abc")
        env = {"BLAZE_SHARED_TARGET": "1", "BLAZE_TARGET_DIR": os.path.join(base, "shared_target")}
        got = {
            "coldbuild_cargo_first": timed(["cargo", "build"], a),
            "coldbuild_cargo_second": timed(["cargo", "build"], b),
        }
        timed([BLAZE, "build"], a, env)  # the first project populates the shared target
        got["coldbuild_blaze_shared_second"] = timed([BLAZE, "build"], c, env)
        for k, v in got.items():
            best[k] = min(best.get(k, v), v)
    return best


if __name__ == "__main__":
    out = measure(int(sys.argv[1]) if len(sys.argv) > 1 else 2)
    path = os.path.join(HERE, "results.json")
    data = json.load(open(path))
    data.update(out)
    json.dump(data, open(path, "w"), indent=2)
    print(json.dumps(out, indent=2))
