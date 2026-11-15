#!/usr/bin/env python3
"""Render an original, data-driven V8 v7 race film from bench.py recordings.

Does not run benchmarks. Input is one frozen no-JIT campaign; all measurement
and correctness checks happen through scripts/benchmark/bench.py.
"""
import argparse
import functools
import hashlib
import json
import math
import statistics
import subprocess
import wave
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

W, H, FPS, DURATION = 960, 540, 30, 76
INK = "#100f24"
WHITE = "#f4f0ff"
MUTED = "#aaa5c7"
GOLD = "#ffd34d"
COLORS = {"oxide": "#ff875e", "quickjs": "#b9d3e4", "jsc": "#a899ff",
          "v8": "#54d6ee", "boa": "#e67fa8", "brimstone": "#a4dc7e",
          "mujs": "#e4bc80", "duktape": "#7f9bd9"}
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
              "", "The relay concatenates per-program median process times, giving each program one eighth of the track. Position within a program is a disclosed linear illustration, not a recorded iteration trace or live race. The freeze happens when oxide finishes; cars still racing have not finished. Relay elapsed time is a different metric from the geometric-mean scoreboard.",
              "", "## Individual programs", "", "| Program | " + " | ".join(NAMES[n] for n in ORDER) + " |", "| --- | " + " | ".join("---:" for _ in ORDER) + " |"]
    for i, title in enumerate(TITLES):
        lines.append("| " + title + " | " + " | ".join(f"{data['engines'][n]['seconds'][i]:.6f}s" for n in ORDER) + " |")
    lines += ["", "## Reproduction and provenance", "", "The campaign directory contains plan.json, commands.json, immutable executable snapshots, wrapper flags, library/binary hashes, the manifest, raw stdout/stderr/counters, A/A and A/B results and every paired report. Measurement commands use scripts/benchmark/bench.py only. No encoding, compilation, tests or other measurements run alongside a recording.",
              "", "Render with: python3 scripts/promo/v8_race.py --campaign CAMPAIGN --output OUTPUT", "",
              "The animation, car sprites and synthesized soundtrack are original. The visual reference is https://x.com/Ion_Mio_/status/2119684071299436891; no frames, music, sprites or source code from that video are included.", ""]
    (out / "report.md").write_text("\n".join(lines))


@functools.lru_cache(maxsize=32)
def font(size, mono=False):
    family = "DejaVu Sans Mono" if mono else "Noto Sans CJK SC"
    spec = subprocess.check_output(["fc-match", "-f", "%{file}\n%{index}", family], text=True).splitlines()
    return ImageFont.truetype(spec[0], size, index=int(spec[1] or "0"))


@functools.lru_cache(maxsize=4096)
def glyph(text, size, color, mono=False):
    f = font(size, mono)
    box = f.getbbox(text)
    im = Image.new("RGBA", (max(1, box[2] - box[0] + 2), box[3] - box[1] + 3))
    d = ImageDraw.Draw(im)
    d.text((1 - box[0], 1 - box[1]), text, font=f, fill=color)
    return im


def text(im, s, x, y, size=16, color=WHITE, align="left", mono=False):
    spr = glyph(str(s), size, color, mono)
    if align == "center":
        x -= spr.width / 2
    elif align == "right":
        x -= spr.width
    im.paste(spr, (round(x), round(y)), spr)


def panel(im, box, fill="#211e3c", outline="#464063", radius=9):
    ImageDraw.Draw(im).rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=1)


def car(im, x, y, color, scale=1, hero=False, tick=0):
    """Original side-view electric rally car, built from pixel rectangles."""
    spr = Image.new("RGBA", (52, 24))
    d = ImageDraw.Draw(spr)
    d.ellipse((1, 18, 50, 23), fill="#090a18")
    d.polygon([(3, 10), (14, 9), (21, 2), (35, 2), (43, 9), (49, 10), (50, 18), (2, 18)], fill=color)
    d.rectangle((3, 13, 48, 16), fill="#eeeeff")
    d.polygon([(17, 9), (23, 4), (28, 4), (28, 9)], fill="#30395f")
    d.polygon([(30, 4), (34, 4), (40, 9), (30, 9)], fill="#436484")
    d.rectangle((44, 11, 50, 13), fill="#fff1a6")
    d.rectangle((2, 10, 5, 12), fill="#ff4463")
    for xx in [12, 39]:
        d.rectangle((xx - 5, 16, xx + 4, 22), fill="#0e1124")
        d.rectangle((xx - 2, 17, xx + 1, 20), fill="#cad2ea")
        d.point((xx + (int(tick * 8) % 3) - 1, 18), fill="#53637f")
    if hero:
        d.polygon([(24, 11), (27, 8), (30, 11), (27, 14)], fill="#ffb33e")
    if scale != 1:
        spr = spr.resize((round(52 * scale), round(24 * scale)), Image.Resampling.NEAREST)
    im.paste(spr, (round(x), round(y)), spr)


def logo(im, x, y, scale=1):
    d = ImageDraw.Draw(im)
    pts = [(0, -22), (18, -7), (9, 22), (-15, 10), (-18, -5)]
    p = [(x + a * scale, y + b * scale) for a, b in pts]
    d.polygon(p, fill="#ff875e")
    d.polygon([(x, y - 22 * scale), (x + 18 * scale, y - 7 * scale), (x - 4 * scale, y - 2 * scale)], fill=GOLD)
    d.polygon([(x - 4 * scale, y - 2 * scale), (x + 9 * scale, y + 22 * scale), (x - 15 * scale, y + 10 * scale)], fill="#df4f60")


def background(t, road=False):
    im = Image.new("RGB", (W, H), INK)
    d = ImageDraw.Draw(im)
    for y in range(0, 185):
        f = y / 185
        d.line((0, y, W, y), fill=(int(17 + 19 * f), int(15 + 9 * f), int(40 + 31 * f)))
    for i in range(58):
        x, y = (i * 139 + 33) % W, (i * 59 + 7) % 128
        c = "#9685b4" if (i + int(t / 2)) % 4 else "#d6c8ee"
        d.rectangle((x, y, x + 1, y + 1), fill=c)
    d.ellipse((707, 19, 806, 118), fill="#714571")
    for y in range(61, 119, 7):
        d.rectangle((700, y, 812, y + 2), fill="#241638")
    for offset, color in [(0, "#342c56"), (75, "#252543")]:
        points = [(0, 178)] + [(i, 133 + ((i * 13 + offset) % 57)) for i in range(-70, W + 100, 70)] + [(W, 180)]
        d.polygon(points, fill=color)
    if not road:
        for x in range(-500, 1500, 100):
            d.line((480, 180, x, 540), fill="#2e244c")
        for y in [190, 204, 224, 254, 298, 360, 446, 538]:
            d.line((0, y, W, y), fill="#342651")
    d.rectangle((0, 0, W, 4), fill="#ff875e")
    return im


def header(im, title, subtitle="", label="V8 v7  /  NO JIT"):
    text(im, "QUICKJS-OXIDE", 28, 18, 12, GOLD, mono=True)
    panel(im, (733, 15, 932, 43), fill="#25233e", outline="#75673e")
    text(im, label, 832, 20, 13, GOLD, align="center", mono=True)
    text(im, title, 28, 54, 30)
    if subtitle:
        text(im, subtitle, 29, 103, 15, MUTED)


def caption(im, s):
    panel(im, (28, 488, 932, 526), fill="#17162e", outline="#51476b")
    text(im, s, 480, 495, 17, WHITE, align="center")


def progress(seconds, elapsed):
    passed = 0.0
    for i, duration in enumerate(seconds):
        if elapsed < passed + duration:
            return (i + max(0, elapsed - passed) / duration) / 8
        passed += duration
    return 1.0


def race(data, t, countdown=False):
    stop = data["engines"]["oxide"]["relay_seconds"]
    elapsed = max(0, min(1, t / 23)) * stop if not countdown else 0
    frozen = t >= 23 and not countdown
    motion_time = min(t, 23)
    im = background(motion_time, True)
    header(im, "八项接力，跑一遍就知道", "同样代码，同样工作量。每项占赛道 1/8。")
    panel(im, (723, 88, 932, 130), fill="#29233a", outline=GOLD)
    text(im, "停帧 · oxide 完赛" if frozen else f"重放计时  {elapsed:05.2f} s", 828, 96, 16, GOLD, align="center")
    d = ImageDraw.Draw(im)
    left, right, top, lane = 220, 887, 177, 35
    d.rectangle((22, top - 6, 937, top + lane * 8), fill="#262e46")
    for i in range(8):
        y = top + lane * i
        if ORDER[i] == "oxide":
            d.rectangle((23, y, 936, y + lane - 1), fill="#4b3542")
        for x in range(left, right, 28):
            d.line((x, y + lane - 1, x + 10, y + lane - 1), fill="#505974")
    for j in range(9):
        x = round(left + (right - left) * j / 8)
        d.line((x, top - 6, x, top + lane * 8 - 1), fill="#66657b", width=1)
        if j < 8:
            text(im, TITLES[j], x + 3, 155, 9, MUTED)
    for y in range(top - 6, top + lane * 8, 7):
        for x in [right, right + 6]:
            d.rectangle((x, y, x + 5, y + 6), fill=WHITE if ((y - top) // 7 + x // 6) % 2 else "#111529")
    for i, name in enumerate(ORDER):
        e = data["engines"][name]
        y = top + lane * i
        p = progress(e["seconds"], elapsed)
        text(im, NAMES[name], 30, y + 8, 14, COLORS[name], mono=True)
        x = left - 48 + p * (right - left)
        if p < 1 and elapsed > 0:
            for k in range(3):
                d.line((x - 7 - k * 7, y + 19, x - 3 - k * 7, y + 19), fill=COLORS[name])
        car(im, x, y + 3, COLORS[name], hero=name == "oxide", tick=motion_time)
        if p == 1:
            text(im, f"{e['relay_seconds']:.2f}s", 927, y + 11, 10, GOLD, align="right", mono=True)
    caption(im, "按各项实测中位耗时重放 · 段内匀速示意" if not frozen else "oxide 完赛时停帧；接力时间与几何平均榜单是不同指标")
    if countdown:
        count = str(3 - min(2, int(t)))
        panel(im, (411, 232, 549, 367), fill="#17162e", outline=GOLD)
        text(im, count, 480, 233, 92, GOLD, align="center", mono=True)
    elif t < .5:
        text(im, "GO!", 480, 225, 72, GOLD, align="center", mono=True)
    return im


def frame(data, t):
    if 12 <= t < 42:
        return race(data, t - 12 if t < 15 else t - 15, countdown=t < 15)
    im = background(t)
    d = ImageDraw.Draw(im)
    if t < 7:
        logo(im, 91, 102, 1.6)
        text(im, "quickjs-oxide", 143, 68, 48, WHITE, mono=True)
        text(im, "安全 Rust，也能跑得快。", 480, 175, 40, GOLD, align="center")
        text(im, "V8 v7 · 8 个引擎 · 全部关闭 JIT", 480, 245, 24, WHITE, align="center")
        d.rectangle((0, 365, W, 420), fill="#282b43")
        for x in range(-60, W, 60):
            xx = (x - int(t * 95)) % (W + 60) - 60
            d.rectangle((xx, 391, xx + 28, 393), fill="#636279")
        car(im, 270 + min(t / 4, 1) * 160, 337, COLORS["oxide"], scale=3, hero=True, tick=t)
        caption(im, "不是印象分。让相同的代码，跑在相同的机器上。")
    elif t < 12:
        header(im, "八个引擎，同场出发", "解释执行；V8、JSC 均显式关闭 JIT。")
        for i, n in enumerate(ORDER):
            x, y = 28 + (i % 4) * 233, 166 + (i // 4) * 143
            panel(im, (x, y, x + 221, y + 126), outline=COLORS[n])
            car(im, x + 17, y + 15, COLORS[n], scale=1.5, hero=n == "oxide", tick=t)
            text(im, NAMES[n], x + 15, y + 61, 17, COLORS[n], mono=True)
            text(im, VERSIONS[n], x + 15, y + 93, 10, MUTED)
        caption(im, "8 项固定工作量 · 每项 6 轮 · 同轮 QuickJS 对照")
    elif t < 57:
        header(im, "八项综合，相对速度", "QuickJS = 1×；数值越大越快。固定工作量点估计，非原版跑分。")
        maximum = max(e["speed"] for e in data["engines"].values()) * 1.04
        start, span = 322, 488
        line = start + span / maximum
        d.line((line, 157, line, 459), fill="#8d829e", width=1)
        text(im, "1×", line, 139, 11, MUTED, align="center", mono=True)
        reveal = min(1.0, max(0, (t - 42) / 2))
        reveal = 1 - (1 - reveal) ** 3
        for i, n in enumerate(data["rank"]):
            e = data["engines"][n]
            y = 168 + i * 35
            if n == "oxide":
                panel(im, (22, y - 4, 938, y + 30), fill="#49333e", outline=COLORS[n], radius=5)
            text(im, f"{i + 1:02}", 33, y + 3, 14, MUTED, mono=True)
            car(im, 70, y - 1, COLORS[n], hero=n == "oxide")
            text(im, NAMES[n], 132, y + 3, 16, COLORS[n], mono=True)
            d.rounded_rectangle((start, y + 6, start + e["speed"] / maximum * span * reveal, y + 20), radius=3, fill=COLORS[n])
            text(im, f"{e['speed']:.2f}×", 919, y + 1, 21, COLORS[n], align="right", mono=True)
        caption(im, "综合速度 = 各项中位耗时比几何平均的倒数；逐项噪声范围见报告")
    elif t < 65:
        e = data["engines"]["oxide"]
        reduction = (1 - e["time_ratio"]) * 100
        header(im, "和 QuickJS 相比，结果是", "V8 v7 八项固定工作量 · 本次同机实测")
        panel(im, (27, 155, 581, 451), fill="#30243b", outline=COLORS["oxide"])
        text(im, f"{e['speed']:.2f}×", 303, 168, 100, GOLD, align="center", mono=True)
        text(im, "相对速度", 303, 303, 27, WHITE, align="center")
        text(im, f"耗时{'减少' if reduction >= 0 else '增加'} {abs(reduction):.1f}%", 303, 361, 27, COLORS["oxide"], align="center")
        logo(im, 750, 231, 2.1)
        text(im, "0 unsafe", 750, 310, 32, WHITE, align="center", mono=True)
        text(im, "全部关闭 JIT", 750, 363, 22, GOLD, align="center")
        caption(im, "安全 Rust · 寄存器字节码 · 内联缓存 · 正则搜索优化")
    elif t < 72:
        header(im, "测量说明", "2026-11-15 · Ryzen 7 7840HS · Linux x86_64 · CPU 2")
        lines = ["8 项 V8 v7 固定程序；原始任务和循环次数相同。",
                 "每项 6 轮 ABBA；先做同二进制 A/A，全部串行测量。",
                 "新进程计时：包含启动、编译、执行与退出。",
                 "V8 --jitless；JSC 的 JS 与正则 JIT 均关闭。",
                 "综合榜：各项耗时比几何平均的倒数，不是原版跑分。",
                 "赛车：逐项中位耗时接力；段内匀速示意，oxide 完赛时停帧。",
                 "版本、哈希、逐项区间与未通过的增益门槛见随附报告。"]
        panel(im, (27, 151, 933, 465), fill="#1f1d36", outline="#5f537b")
        for i, s in enumerate(lines):
            d.rectangle((47, 170 + i * 41, 52, 175 + i * 41), fill=GOLD if i % 2 == 0 else COLORS["oxide"])
            text(im, s, 66, 160 + i * 41, 17, WHITE if i < 4 else MUTED)
        caption(im, "相同机器、相同任务；展示本次记录的版本与配置。")
    else:
        logo(im, 480, 130, 2.5)
        text(im, "quickjs-oxide", 480, 233, 49, WHITE, align="center", mono=True)
        text(im, "Safe Rust. No JIT. Let's go.", 480, 313, 24, GOLD, align="center", mono=True)
        text(im, "github.com/pocket-nexus/quickjs-oxide", 480, 385, 18, MUTED, align="center", mono=True)
        car(im, 110 + (t - 72) * 155, 439, COLORS["oxide"], scale=1.8, hero=True, tick=t)
    return im


def audio(path):
    """Original 118 BPM chiptune; no sampled or third-party audio."""
    sr, bpm = 48000, 118
    track = np.zeros((DURATION * sr, 2), dtype=np.float32)
    rng = np.random.default_rng(79)

    def tone(start, length, midi, volume, pan=0, kind="lead"):
        begin, length = int(start * sr), int(length * sr)
        length = min(length, len(track) - begin)
        if length <= 0:
            return
        t = np.arange(length, dtype=np.float32) / sr
        hz = 440 * 2 ** ((midi - 69) / 12)
        env = np.minimum(t / .008, 1) * np.maximum(0, 1 - t / (length / sr)) ** .6
        if kind == "lead":
            sig = (np.sin(2 * np.pi * hz * t) + .3 * np.sin(6 * np.pi * hz * t)) * env
        else:
            sig = np.sin(2 * np.pi * hz * t) * env
        track[begin:begin + length, 0] += sig * volume * (1 - pan * .35)
        track[begin:begin + length, 1] += sig * volume * (1 + pan * .35)

    beat = 60 / bpm
    roots = [38, 34, 41, 36]
    melody = [62, 65, 69, 72, 69, 65, 74, 72, 65, 69, 72, 77, 74, 72, 69, 65]
    for b in range(int(DURATION / beat)):
        start = b * beat
        root = roots[(b // 8) % 4]
        tone(start, beat * .8, root, .15, kind="bass")
        if start < 65:
            for half in range(2):
                tone(start + half * beat / 2, beat * .38, melody[(b * 2 + half) % len(melody)], .045, .5 if half else -.5)
        n = min(int(.16 * sr), len(track) - int(start * sr))
        t = np.arange(n) / sr
        kick = np.sin(2 * np.pi * (45 * t + 38 * .035 * (1 - np.exp(-t / .035)))) * np.exp(-t * 28) * .22
        track[int(start * sr):int(start * sr) + n] += kick[:, None]
        if b % 2:
            noise = rng.uniform(-1, 1, n) * np.exp(-t * 34) * .07
            track[int(start * sr):int(start * sr) + n] += noise[:, None]
    for start in [12, 13, 14]:
        tone(start, .13, 76, .16)
    tone(15, .5, 88, .15)
    for n in [62, 65, 69, 74]:
        tone(72 + (n - 62) / 20, 2.4, n, .08)
    fade = np.minimum(np.arange(len(track)) / sr, 1) * np.minimum((len(track) - np.arange(len(track))) / sr / 1.2, 1)
    track *= fade[:, None]
    track = np.tanh(track) * .85
    with wave.open(str(path), "wb") as f:
        f.setnchannels(2)
        f.setsampwidth(2)
        f.setframerate(sr)
        f.writeframes((track * 32767).astype("<i2").tobytes())


def render(data, out, stills_only=False):
    out.mkdir(parents=True, exist_ok=True)
    stills = out / "stills"
    stills.mkdir(exist_ok=True)
    times = [5, 10, 13, 22, 40, 51, 61, 68, 74]
    contact = Image.new("RGB", (1440, 810), INK)
    for i, t in enumerate(times):
        im = frame(data, t)
        im.resize((1920, 1080), Image.Resampling.NEAREST).save(stills / f"{t:02}.png")
        contact.paste(im.resize((480, 270), Image.Resampling.LANCZOS), ((i % 3) * 480, (i // 3) * 270))
    contact.save(out / "contact.png")
    frame(data, 5).resize((1920, 1080), Image.Resampling.NEAREST).save(out / "poster.png")
    if stills_only:
        return
    audio(out / "soundtrack.wav")
    command = ["ffmpeg", "-hide_banner", "-y", "-f", "rawvideo", "-pixel_format", "rgb24",
               "-video_size", f"{W}x{H}", "-framerate", str(FPS), "-i", "pipe:0",
               "-i", str(out / "soundtrack.wav"), "-vf", "scale=1920:1080:flags=neighbor",
               "-c:v", "libx264", "-preset", "medium", "-crf", "19", "-threads", "2",
               "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-ar", "48000",
               "-af", "loudnorm=I=-18:TP=-1.5:LRA=11", "-movflags", "+faststart",
               "-shortest", str(out / "quickjs-oxide-v8-v7-no-jit-zh.mp4")]
    (out / "encode-command.json").write_text(json.dumps(command, indent=2) + "\n")
    with (out / "encode.log").open("w") as log:
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=log, stderr=log)
        try:
            for number in range(DURATION * FPS):
                proc.stdin.write(frame(data, number / FPS).tobytes())
                if number % (FPS * 5) == 0:
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
    write_report(data, args.output)
    render(data, args.output, args.stills_only)
    print("video artifacts: " + str(args.output.resolve()), flush=True)


if __name__ == "__main__":
    main()
