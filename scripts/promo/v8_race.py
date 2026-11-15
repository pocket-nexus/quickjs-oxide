#!/usr/bin/env python3
"""Render only the scrolling eight-leg V8 v7 relay from frozen recordings.

Does not run benchmarks. Input is one frozen no-JIT campaign; all measurement
and correctness checks happen through scripts/benchmark/bench.py.
"""
import argparse
import hashlib
import json
import math
import statistics
import subprocess
from pathlib import Path

from PIL import Image
from relay import Scene, audio, W, H, FPS, DURATION, P, GO, RATE

NAMES = {"oxide": "quickjs-oxide", "quickjs": "QuickJS", "jsc": "JSC",
         "v8": "V8", "boa": "Boa", "brimstone": "Brimstone",
         "mujs": "MuJS", "duktape": "Duktape"}
VERSIONS = {"oxide": "df85cbfc · safe Rust", "quickjs": "2026-06-04",
            "jsc": "WebKitGTK 2.54.2", "v8": "14.6 · Node 26.13.1",
            "boa": "0.22 · f0789a20", "brimstone": "697465b0",
            "mujs": "1.3.11", "duktape": "2.7.0"}
ORDER = ["oxide", "quickjs", "jsc", "v8", "boa", "brimstone", "mujs", "duktape"]
CASES = ["richards", "deltablue", "crypto", "raytrace", "earley-boyer",
         "regexp", "splay", "navier-stokes"]
TITLES = ["Richards", "DeltaBlue", "Crypto", "RayTrace", "EarleyBoyer",
          "RegExp", "Splay", "NavierStokes"]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text())


def campaign(root):
    """Refuse incomplete, mixed-workload or JIT-enabled inputs."""
    plan = read_json(root / "plan.json")
    if "No JIT" not in plan["mode"]:
        raise ValueError("this film requires the declared no-JIT campaign")
    manifest = read_json(root / "suite/manifest.json")
    workloads = manifest["metadata"]["workloads"]["workloads"]
    expected = {"v8-v7-" + c for c in CASES}
    if {w["case"] for w in workloads} != expected:
        raise ValueError("all eight V8 v7 programs are required")
    hashes = {w["case"]: w["sha256"] for w in workloads}
    identities = read_json(root / "identities.json")
    for name, identity in identities.items():
        path = identity.get("snapshot") or (name if name.startswith("/") else None)
        if path and sha(path) != identity["sha256"]:
            raise ValueError(f"recorded executable or library changed: {name}")
    inputs = {}

    def load_run(name, engines):
        path = root / name / "results.json"
        result = read_json(path)
        meta = result["metadata"]
        if set(meta["engines"]) != set(engines):
            raise ValueError(f"{name}: unexpected engines")
        for label, engine in meta["engines"].items():
            identity = identities["quickjs" if label == "control" else label]
            if engine["sha256"] != identity["sha256"]:
                raise ValueError(f"{name}: binary differs from the frozen identity")
        if meta["repeat"] != 6 or meta["order_strategy"] != "abba" or meta["cpu"] != 2:
            raise ValueError(f"{name}: measurement plan changed")
        if {w["case"]: w["sha256"] for w in meta["workloads"]} != hashes:
            raise ValueError(f"{name}: workload bytes differ")
        samples = result["samples"]
        if len(samples) != 96 or any(s["status"] != "ok" for s in samples):
            raise ValueError(f"{name}: failed or missing samples")
        keys = {(s["case"], s["engine"], s["repetition"]) for s in samples}
        if len(keys) != 96:
            raise ValueError(f"{name}: duplicate samples")
        rows = {(s["case"], s["engine"]): s for s in result["summary"]}
        inputs[name] = {"path": str(path.resolve()), "sha256": sha(path)}
        return result, rows

    aa, aa_rows = load_run("aa", ["quickjs", "control"])
    data = {"date": plan["date"], "mode": "NO JIT", "plan": plan,
            "programs": TITLES, "engines": {}, "inputs": inputs}
    for name in ORDER:
        if name == "quickjs":
            continue
        result, rows = load_run("ab-" + name, ["quickjs", name])
        medians = [rows["v8-v7-" + c, name]["median_wall_ns"] / 1e9 for c in CASES]
        reference = [rows["v8-v7-" + c, "quickjs"]["median_wall_ns"] / 1e9 for c in CASES]
        ratio = math.exp(statistics.mean(math.log(a / b) for a, b in zip(medians, reference)))
        paired_path = root / ("paired-" + name + ".json")
        paired = read_json(paired_path)
        if paired["candidate"] != name or paired["baseline"] != "quickjs":
            raise ValueError("paired report names differ")
        engine = result["metadata"]["engines"][name]
        version = engine["version"]["stdout"]
        if name == "v8" and "--jitless" not in version:
            raise ValueError("V8 is not explicitly jitless")
        if name == "jsc" and not all(x in version for x in ["--useJIT=false", "--useRegExpJIT=false"]):
            raise ValueError("JSC JavaScript and RegExp JIT must both be disabled")
        data["engines"][name] = {"seconds": medians, "reference_seconds": reference,
                                  "time_ratio": ratio, "speed": 1 / ratio,
                                  "relay_seconds": sum(medians),
                                  "per_program_gain_gate": paired["gate_passed"],
                                  "gate_cases_passed": sum(r.get("gate_passed") is True for r in paired["results"]),
                                  "sha256": engine["sha256"], "version": VERSIONS[name]}
        inputs["paired-" + name] = {"path": str(paired_path.resolve()), "sha256": sha(paired_path)}
        if name == "oxide":
            data["engines"]["quickjs"] = {"seconds": reference, "time_ratio": 1.0, "speed": 1.0,
                                               "relay_seconds": sum(reference), "version": VERSIONS["quickjs"]}
    aa_ratios = [aa_rows["v8-v7-" + c, "control"]["median_wall_ns"] /
                 aa_rows["v8-v7-" + c, "quickjs"]["median_wall_ns"] for c in CASES]
    data["aa_time_ratio"] = math.exp(statistics.mean(map(math.log, aa_ratios)))
    data["sample_count"] = len(aa["samples"]) * 8
    data["identities"] = identities
    data["rank"] = sorted(ORDER, key=lambda n: data["engines"][n]["speed"], reverse=True)
    return data


def write_report(data, out):
    out.mkdir(parents=True, exist_ok=True)
    (out / "video-data.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    lines = ["# V8 v7 race video — no JIT", "", "Measured " + data["date"] + ".",
             "", "Eight fixed-work programs, six ABBA pairs per engine against QuickJS; separate same-binary A/A first. CPU 2, serial Linux runs, process time includes startup, compilation, execution and exit. All 768 formal samples passed stdout and exit checks.",
             "", "This is a new baseline: only the modern print shim was replaced by an ES5 shim, on every engine, for MuJS and Duktape. Benchmark bodies and fixed iteration counts are unchanged. No older campaign's timings are spliced into this one.",
             "", "| Engine | Pinned version | Time / paired QuickJS | Relative speed | All 8 per-case gain gates |",
             "| --- | --- | ---: | ---: | --- |"]
    for n in data["rank"]:
        e = data["engines"][n]
        gate = "reference" if n == "quickjs" else ("pass" if e["per_program_gain_gate"] else "not passed")
        lines.append(f"| {NAMES[n]} | {VERSIONS[n]} | {e['time_ratio']:.6f} | {e['speed']:.6f}× | {gate} |")
    lines += ["", "Relative speed is the reciprocal of the equal-weight geometric mean of each program's median time ratio to its paired QuickJS run. It is not the original suite's adaptive score. These are point estimates; per-case bootstrap intervals and A/A noise are in the preserved paired reports. A missed all-case gain gate remains a miss.",
              "", f"A/A geometric-mean time ratio: {data['aa_time_ratio']:.6f}.",
              "", "V8 is the Node.js CLI with --jitless. JSC is the WebKitGTK jsc shell with --useJIT=false --useRegExpJIT=false. Boa uses --optimize; Brimstone uses --annex-b. Duktape uses a minimal C print/file host over the system library. Runtime code for oxide is df85cbfc (identical to 5809ffad).",
              "", "The film contains only the eight-leg relay. A camera follows oxide through a scrolling track with parallax mountains, stage signs and off-screen car indicators. Each leg occupies one eighth of the track; per-program median process times drive uniformly interpolated motion, at a fixed 0.4× rate. The scene freezes when oxide finishes; the time column reports every engine's recorded relay total, including those still behind. No geometric-mean scoreboard or other scene appears in this cut.",
              "", "## Individual programs", "", "| Program | " + " | ".join(NAMES[n] for n in ORDER) + " |", "| --- | " + " | ".join("---:" for _ in ORDER) + " |"]
    for i, title in enumerate(TITLES):
        lines.append("| " + title + " | " + " | ".join(f"{data['engines'][n]['seconds'][i]:.6f}s" for n in ORDER) + " |")
    lines += ["", "## Reproduction and provenance", "", "The campaign directory contains plan.json, commands.json, immutable executable snapshots, wrapper flags, library/binary hashes, the manifest, raw stdout/stderr/counters, A/A and A/B results and every paired report. Measurement commands use scripts/benchmark/bench.py only. No encoding, compilation, tests or other measurements run alongside a recording.",
              "", "Render with: python3 scripts/promo/v8_race.py --campaign CAMPAIGN --output OUTPUT", "",
              "The visual reference is https://x.com/Ion_Mio_/status/2119684071299436891. The small car construction is adapted from the Apache-2.0 MoeForge/moejs race kit, with attribution in scripts/promo/assets/README.md. The new scene uses rasterized Fusion Pixel 12px glyphs under OFL-1.1. No frames or audio from the reference video are included; the soundtrack is synthesized for this cut.", ""]
    (out / "report.md").write_text("\n".join(lines))


def render(data, out, scene, stills_only=False):
    out.mkdir(parents=True, exist_ok=True)
    stills = out / "stills"
    stills.mkdir(exist_ok=True)
    times = [0, 1.6] + [(a + b) / 2 for a, b in zip(scene.stage_times, scene.stage_times[1:])] + [scene.freeze_at + .3, 25.5]
    contact = Image.new("RGB", (W * 3, H * 4), P["ink"])
    for i, t in enumerate(times):
        im = scene.frame(t)
        im.resize((1920, 1080), Image.Resampling.NEAREST).save(stills / f"{t:05.2f}.png")
        contact.paste(im, ((i % 3) * W, (i // 3) * H))
    contact.save(out / "contact.png")
    scene.frame(scene.stage_times[4] + .2).resize((1920, 1080), Image.Resampling.NEAREST).save(out / "poster.png")
    if stills_only:
        return
    audio(out / "soundtrack.wav", scene)
    command = ["ffmpeg", "-hide_banner", "-y", "-f", "rawvideo", "-pixel_format", "rgb24",
               "-video_size", f"{W}x{H}", "-framerate", str(FPS), "-i", "pipe:0",
               "-i", str(out / "soundtrack.wav"), "-vf", "scale=1920:1080:flags=neighbor",
               "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-threads", "2",
               "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-ar", "48000",
               "-af", "loudnorm=I=-18:TP=-1.5:LRA=11", "-movflags", "+faststart",
               "-shortest", str(out / "quickjs-oxide-v8-v7-relay-only-no-jit-zh.mp4")]
    (out / "encode-command.json").write_text(json.dumps(command, indent=2) + "\n")
    with (out / "encode.log").open("w") as log:
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=log, stderr=log)
        try:
            for number in range(DURATION * FPS):
                proc.stdin.write(scene.frame(number / FPS).tobytes())
                if number % (FPS * 2) == 0:
                    print(f"rendered {number / FPS:.0f}/{DURATION}s", flush=True)
        finally:
            proc.stdin.close()
        if proc.wait() != 0:
            raise RuntimeError("ffmpeg failed; inspect encode.log")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--campaign", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--stills-only", action="store_true")
    args = parser.parse_args()
    data = campaign(args.campaign.resolve())
    scene = Scene(data)
    data["render"] = {"cut": "eight-leg relay only", "width": 1920, "height": 1080,
                      "native_pixels": [W, H], "fps": FPS, "duration": DURATION,
                      "go_at": GO, "playback_rate": RATE, "freeze_at": scene.freeze_at,
                      "relay_rank": sorted(ORDER, key=lambda n: data["engines"][n]["relay_seconds"])}
    write_report(data, args.output)
    render(data, args.output, scene, args.stills_only)
    print("relay artifacts: " + str(args.output.resolve()), flush=True)


if __name__ == "__main__":
    main()
