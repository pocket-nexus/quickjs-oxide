"""A scrolling eight-leg relay, rendered on a 480×270 pixel canvas.

The small rally-car construction is adapted from MoeForge/moejs,
bench/race/pix.py at 3035a4d642967b2eddeee8164ef3010ec85fc401,
Licensed under Apache-2.0. See assets/README.md for attribution.
The scene, camera, timing integration and soundtrack are quickjs-oxide's.
"""
import functools
import json
import math
import random
import wave
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

W, H, FPS, DURATION = 480, 270, 60, 26
GO, RATE = 2.2, .4
ROAD, LANE, START, LEG = 68, 23, 150, 240
BOTTOM, FINISH = ROAD + 8 * LANE, START + 8 * LEG
P = dict(ink="#1a1c2c", plum="#5d275d", red="#b13e53", orange="#ef7d57",
         yellow="#ffcd75", lime="#a7f070", green="#38b764", teal="#257179",
         navy="#29366f", blue="#3b5dc9", sky="#41a6f6", cyan="#73eff7",
         white="#f4f4f4", grey="#94b0c2", slate="#566c86", dark="#333c57")
ORDER = ["oxide", "quickjs", "jsc", "v8", "boa", "brimstone", "mujs", "duktape"]
NAMES = dict(oxide="quickjs-oxide", quickjs="QuickJS", jsc="JSC", v8="V8",
             boa="Boa", brimstone="Brimstone", mujs="MuJS", duktape="Duktape")
COLORS = dict(oxide=P["orange"], quickjs=P["grey"], jsc="#b6a1ed", v8=P["cyan"],
              boa="#e07d9c", brimstone=P["lime"], mujs=P["yellow"], duktape=P["blue"])
STAGES = ["任务调度", "约束求解", "加密解密", "光线追踪", "解析与推理", "正则匹配", "树操作", "流体模拟"]
ATLAS = json.loads((Path(__file__).parent / "assets/pixel-font.json").read_text())["glyphs"]


@functools.lru_cache(maxsize=2048)
def lettering(value, color=P["white"], scale=1, outline=True):
    """Use only the fixed 12px glyph grid and integer scaling."""
    glyphs = [ATLAS[c] for c in str(value)]
    visible = [g for g in glyphs if g["h"]]
    top = min((g["y"] for g in visible), default=0)
    bottom = max((g["y"] + g["h"] for g in visible), default=12)
    mask = Image.new("L", (sum(g["advance"] for g in glyphs) + 4, bottom - top + 4))
    d = ImageDraw.Draw(mask)
    off = 2
    for g in glyphs:
        for y, bits in enumerate(g["rows"]):
            for x in range(g["w"]):
                if bits & (1 << (g["w"] - 1 - x)):
                    d.point((off + g["x"] + x, 2 + g["y"] - top + y), fill=255)
        off += g["advance"]
    image = Image.new("RGBA", mask.size)
    if outline:
        image.paste(P["ink"], (0, 0), mask.filter(ImageFilter.MaxFilter(3)))
    image.paste(color, (0, 0), mask)
    if scale > 1:
        image = image.resize((image.width * scale, image.height * scale), Image.Resampling.NEAREST)
    return image


def paste(image, sprite, x, y, anchor="lt"):
    if anchor[0] == "m":
        x -= sprite.width // 2
    elif anchor[0] == "r":
        x -= sprite.width
    if anchor[1] == "m":
        y -= sprite.height // 2
    elif anchor[1] == "b":
        y -= sprite.height
    image.alpha_composite(sprite, (round(x), round(y)))


def label(image, text, x, y, color=P["white"], scale=1, anchor="lt", outline=True):
    paste(image, lettering(text, color, scale, outline), x, y, anchor)


DOOR = {
    "R": ["##.", "#.#", "##.", "#.#", "#.#"],
    "s": ["...", ".##", "##.", "..#", "##."],
    "Q": [".#.", "#.#", "#.#", ".##", "..#"],
    "J": ["###", "..#", "..#", "#.#", ".#."],
    "V": ["#.#", "#.#", "#.#", "#.#", ".#."],
    "B": ["##.", "#.#", "##.", "#.#", "##."],
    "M": ["#.#", "###", "###", "#.#", "#.#"],
    "D": ["##.", "#.#", "#.#", "#.#", "##."],
}


@functools.lru_cache(maxsize=64)
def car(name, phase=0):
    """Adapted 40×18 car sprite; distinct door badges identify our engines."""
    body = COLORS[name]
    dark = P["red"] if name == "oxide" else P["slate"]
    tag = dict(oxide="Rs", quickjs="Q", jsc="J", v8="V", boa="B", brimstone="B", mujs="M", duktape="D")[name]
    im = Image.new("RGBA", (40, 18))
    d = ImageDraw.Draw(im)
    d.polygon([(10, 6), (14, 1), (27, 1), (32, 6)], fill=body)
    d.polygon([(15, 2), (26, 2), (29, 5), (13, 5)], fill=P["cyan"])
    d.line((21, 2, 21, 5), fill=dark)
    d.rectangle((0, 1, 3, 3), fill=dark)
    d.line((2, 3, 2, 6), fill=dark)
    d.rectangle((1, 6, 37, 13), fill=body)
    d.rectangle((37, 8, 39, 13), fill=body)
    d.line((1, 13, 39, 13), fill=dark)
    d.line((2, 7, 36, 7), fill=P["white"])
    d.rectangle((38, 9, 39, 10), fill=P["yellow"])
    d.rectangle((1, 8, 1, 10), fill=P["red"])
    d.rectangle((16, 6, 26, 13), fill=P["white"])
    gx = 16 + (12 - (4 * len(tag) - 1)) // 2
    for c in tag:
        for yy, row in enumerate(DOOR[c]):
            for xx, bit in enumerate(row):
                if bit == "#":
                    d.point((gx + xx, 7 + yy), fill=P["ink"])
        gx += 4
    for wx in (5, 27):
        d.ellipse((wx, 11, wx + 7, 17), fill=P["ink"])
        hx, hy = [(wx + 3, 13), (wx + 4, 14), (wx + 3, 15), (wx + 2, 14)][phase % 4]
        d.point((hx, hy), fill=P["grey"])
        d.point((wx + 3, 14), fill=P["slate"])
    if name == "quickjs":
        d.line([(1, 6), (10, 6), (14, 1), (27, 1), (32, 6), (37, 6)], fill=P["white"])
        for x in (3, 8, 29, 34):
            d.line((x, 9, x + 3, 12), fill=P["slate"])
    return im


def position(seconds, elapsed):
    off = 0
    for i, duration in enumerate(seconds):
        if elapsed < off + duration:
            return i + max(0, elapsed - off) / duration
        off += duration
    return 8.0


@functools.lru_cache(maxsize=3)
def mountains(color, base, height, seed, snow=False):
    im = Image.new("RGBA", (4096, 66))
    d = ImageDraw.Draw(im)
    for x in range(4096):
        wave = .62 * math.sin(x * .024 + seed) + .28 * math.sin(x * .061 + 1.2 + seed) + .1 * math.sin(x * .17)
        h = base + height * wave
        y = round(66 - h)
        d.line((x, y, x, 66), fill=color)
        if snow and wave > .48:
            d.line((x, y, x, y + 1), fill=P["white"])
    return im


@functools.lru_cache(maxsize=1)
def sky():
    im = Image.new("RGBA", (W, H), P["ink"])
    d = ImageDraw.Draw(im)
    bands = [(0, P["navy"]), (17, P["plum"]), (39, P["red"]), (57, P["orange"]), (66, P["orange"])]
    for (start, color), (end, next_color) in zip(bands, bands[1:]):
        d.rectangle((0, start, W, end), fill=color)
        for y in [end - 2, end - 1]:
            for x in range(y % 2, W, 2):
                d.point((x, y), fill=next_color)
    return im


class Scene:
    def __init__(self, data):
        self.data = data
        self.engines = data["engines"]
        self.stop = self.engines["oxide"]["relay_seconds"]
        self.freeze_at = GO + self.stop / RATE
        if self.freeze_at + 4 > DURATION:
            raise ValueError("recorded relay no longer fits the declared fixed-rate cut")
        self.rank = 1 + sum(self.engines[n]["relay_seconds"] < self.stop for n in ORDER if n != "oxide")
        self.stage_times = [GO]
        for duration in self.engines["oxide"]["seconds"]:
            self.stage_times.append(self.stage_times[-1] + duration / RATE)
        self.clouds = [(random.Random(i + 41).randrange(1400), random.Random(i + 31).randrange(6, 43), 1 + i % 2) for i in range(8)]
        self.confetti = [(random.Random(6 * i).uniform(-90, 90), random.Random(6 * i + 1).uniform(-120, -35), i % 6, i % 2) for i in range(100)]

    def clock(self, t):
        if t >= self.freeze_at:
            return self.stop
        return max(0, t - GO) * RATE

    def front(self, name, elapsed):
        e = self.engines[name]
        x = START + LEG * position(e["seconds"], elapsed)
        if elapsed > e["relay_seconds"]:
            x += 57 * (1 - math.exp(-(elapsed - e["relay_seconds"]) * 3))
        return x

    def camera(self, elapsed):
        return max(0, min(FINISH - 300, self.front("oxide", elapsed) - 300))

    def landscape(self, image, cam, t):
        d = ImageDraw.Draw(image)
        for wx, y, size in self.clouds:
            x = (wx - cam * .05 - t * 2) % 740 - 120
            for dx, dy, radius in [(0, 0, 4), (5, -2, 5), (10, 0, 4), (3, 2, 4), (8, 2, 4)]:
                r = radius * size / 2 + 1
                d.ellipse((x + dx * size - r, y + dy - r, x + dx * size + r, y + dy + r), fill=P["white"])
        for k, color, base, height, seed, snow in [(.08, P["slate"], 26, 18, .5, True), (.25, P["teal"], 15, 8, 2, False), (.5, P["green"], 5, 4, 4, False)]:
            strip = mountains(color, base, height, seed, snow)
            x = int(cam * k)
            image.alpha_composite(strip.crop((x, 0, x + W, 66)), (0, 0))
        d.rectangle((0, 66, W, ROAD - 1), fill=P["green"])
        d.rectangle((0, ROAD, W, BOTTOM - 1), fill=P["dark"])
        d.rectangle((0, BOTTOM + 3, W, H), fill=P["green"])
        for y in [ROAD - 4, BOTTOM]:
            for wx in range(int(cam) // 8 * 8 - 8, int(cam) + W + 8, 8):
                x = wx - int(cam)
                d.rectangle((x, y, x + 7, y + 3), fill=P["white"] if wx // 8 % 2 else P["red"])
        for i in range(1, 8):
            y = ROAD + i * LANE
            for wx in range(int(cam) // 24 * 24 - 24, int(cam) + W + 24, 24):
                x = wx - int(cam)
                d.line((x, y, x + 5, y), fill=P["slate"])

    def gates(self, image, cam, t):
        d = ImageDraw.Draw(image)
        for i in range(9):
            x = round(START + LEG * i - cam)
            if x < -65 or x > W + 65:
                continue
            if i == 8:
                for y in range(ROAD, BOTTOM, 3):
                    for j in range(3):
                        d.rectangle((x + j * 3, y, x + j * 3 + 2, y + 2), fill=P["white"] if ((y - ROAD) // 3 + j) % 2 else P["ink"])
                d.line((x + 4, 40, x + 4, ROAD - 4), fill=P["grey"], width=2)
                for yy in range(4):
                    for xx in range(6):
                        wave = int(t * 8 + xx / 2) % 2
                        d.rectangle((x + 6 + xx * 3, 40 + yy * 3 + wave, x + 8 + xx * 3, 42 + yy * 3 + wave), fill=P["white"] if (xx + yy) % 2 else P["ink"])
            else:
                d.line((x, ROAD, x, BOTTOM - 1), fill=P["white"], width=1)
                d.line((x, 58, x, ROAD - 4), fill=P["grey"], width=2)
                sign = lettering(STAGES[i], P["yellow"], outline=False)
                d.rectangle((x - sign.width // 2 - 3, 42, x + sign.width // 2 + 3, 59), fill=P["ink"], outline=P["slate"])
                paste(image, sign, x, 43, "mt")

    def racers(self, image, cam, elapsed, t, frozen):
        d = ImageDraw.Draw(image)
        for i, name in enumerate(ORDER):
            e = self.engines[name]
            y = ROAD + i * LANE
            front = round(self.front(name, elapsed) - cam)
            done = elapsed >= e["relay_seconds"]
            moving = t >= GO and not frozen and (not done or elapsed - e["relay_seconds"] < .7)
            if front - 41 > max(0, START - cam):
                d.rectangle((max(0, round(START - cam)), y + LANE - 3, min(W, front - 41), y + LANE - 2), fill=COLORS[name])
            caption = NAMES[name] + (f" #{self.rank}" if name == "oxide" and frozen else "")
            if front < 43:
                d.polygon([(2, y + 12), (6, y + 8), (6, y + 16)], fill=COLORS[name])
                label(image, caption, 8, y + 11, P["grey"] if name != "oxide" else P["white"], anchor="lm")
                continue
            if front > W + 36:
                d.polygon([(W - 2, y + 12), (W - 6, y + 8), (W - 6, y + 16)], fill=COLORS[name])
                label(image, caption, W - 8, y + 11, COLORS[name], anchor="rm")
                continue
            cy = y + 4 - (int(t * 18 + i) % 2 if moving else 0)
            if moving:
                for k in range(3):
                    age = (t * 7 + k / 3 + i * .12) % 1
                    xx, yy = front - 42 - age * 12, cy + 14 - age * 4
                    d.rectangle((xx, yy, xx + 1, yy + 1), fill=P["grey"] if age < .45 else P["slate"])
                if name == "oxide":
                    for k in range(3):
                        xx = front - 45 - (t * 150 + 19 * k) % 45
                        d.line((xx, cy + 5 + k * 3, xx + 6, cy + 5 + k * 3), fill=P["white"])
            paste(image, car(name, int(t * 30) % 4 if moving else 0), front - 40, cy)
            lab = lettering(caption, P["white"] if name == "oxide" else P["grey"])
            if front - 44 - lab.width >= 2:
                paste(image, lab, front - 44, cy + 9, "rm")
            elif front + lab.width + 5 < W:
                paste(image, lab, front + 4, cy + 9, "lm")
            if done and not frozen:
                label(image, "完赛", front + 4, cy + 9, P["yellow"], anchor="lm")

    def frame(self, t):
        elapsed = self.clock(t)
        frozen = t >= self.freeze_at
        motion = min(t, self.freeze_at)
        cam = self.camera(elapsed)
        image = sky().copy()
        self.landscape(image, cam, motion)
        self.gates(image, cam, motion)
        self.racers(image, cam, elapsed, motion, frozen)
        d = ImageDraw.Draw(image)
        stage = min(7, int(position(self.engines["oxide"]["seconds"], elapsed)))
        heading = "完成接力" if frozen else ("八项接力" if t < GO else STAGES[stage])
        age = t - self.stage_times[stage] if t >= GO and not frozen else 1
        scale = 1 if 0 <= age < .10 else 2
        y = 3 + round(4 * (1 - min(1, max(0, age) / .16)))
        label(image, heading, 240, y, P["yellow"] if frozen else P["white"], scale, "mt")
        label(image, f"{elapsed:.2f}秒", W - 5, 3, P["yellow"], 2, "rt")
        if frozen:
            badge = lettering("定格", P["white"], 2)
            d.rectangle((4, 3, badge.width + 8, badge.height + 7), fill=P["blue"], outline=P["ink"], width=2)
            paste(image, badge, 6, 4)
        else:
            label(image, "quickjs-oxide", 4, 5, P["white"])
            label(image, "V8 v7", 4, 21, P["yellow"])
        if t < GO + .65:
            d.rectangle((199, 38, 281, 63), fill=P["ink"], outline=P["slate"], width=2)
            for i, when in enumerate([.4, .95, 1.5]):
                on = t >= when
                color = P["lime"] if t >= GO else (P["red"] if on else P["dark"])
                x = 216 + i * 24
                d.ellipse((x - 7, 43, x + 7, 57), fill=color, outline=P["ink"])
                if on:
                    d.point((x - 2, 45), fill=P["white"])
            if t >= GO:
                label(image, "GO!", 240, 143, P["yellow"], 4, "mm")
        if frozen:
            dt = t - self.freeze_at
            if dt < 3:
                colors = [P["yellow"], P["orange"], P["cyan"], P["white"], P["lime"], P["red"]]
                for vx, vy, color, size in self.confetti:
                    x = 300 + vx * dt
                    y = 57 + vy * dt + 66 * dt * dt
                    d.rectangle((x, y, x + 1 + size, y + 2), fill=colors[color])
            if dt >= .65:
                label(image, "用时", W - 3, ROAD - 17, anchor="rt")
                for i, name in enumerate(ORDER):
                    sp = lettering(f"{self.engines[name]['relay_seconds']:.2f}秒", P["yellow"] if name == "oxide" else P["white"])
                    y = ROAD + i * LANE + LANE // 2
                    d.rectangle((W - 5 - sp.width, y - 8, W, y + 8), fill=P["ink"])
                    paste(image, sp, W - 3, y, "rm")
            if dt >= 1.1:
                gap = self.engines["quickjs"]["relay_seconds"] - self.stop
                result = lettering(f"先于 QuickJS {gap:.2f} 秒", P["yellow"], 2)
                x, y = (W - result.width) // 2, 215
                d.rectangle((x - 6, y - 5, x + result.width + 5, y + result.height + 4), fill=P["ink"])
                d.rectangle((x - 3, y - 2, x + result.width + 2, y + result.height + 1), fill=P["red"])
                d.line((x - 2, y - 1, x + result.width + 1, y - 1), fill=P["orange"])
                paste(image, result, x, y)
        mx, mw, my = 141, 198, 263
        d.rectangle((mx - 3, my - 4, mx + mw + 3, my + 4), fill=P["ink"])
        for k in range(1, 8):
            x = mx + mw * k // 8
            d.line((x, my - 3, x, my + 3), fill=P["slate"])
        for name in [n for n in ORDER if n != "oxide"] + ["oxide"]:
            x = mx + mw * position(self.engines[name]["seconds"], elapsed) / 8
            d.rectangle((x - 2, my - 2, x + 2, my + 2), fill=COLORS[name], outline=P["white"] if name == "oxide" else P["ink"])
        label(image, "NO JIT  0.4×", 3, 254, P["ink"], outline=False)
        label(image, "中位示意", W - 3, 254, P["ink"], anchor="rt", outline=False)
        return image.convert("RGB")


def audio(path, scene):
    """Original chiptune, countdown, motor hum, stage cues and finish arpeggio."""
    sr, bpm = 48000, 132
    freeze_at = scene.freeze_at
    track = np.zeros((DURATION * sr, 2), dtype=np.float32)
    rng = np.random.default_rng(79)

    def note(start, length, midi, volume, pan=0):
        at, count = int(start * sr), min(int(length * sr), len(track) - int(start * sr))
        if count <= 0:
            return
        t = np.arange(count, dtype=np.float32) / sr
        hz = 440 * 2 ** ((midi - 69) / 12)
        env = np.minimum(t / .006, 1) * np.maximum(0, 1 - t / (count / sr)) ** .6
        sig = (np.sin(2 * np.pi * hz * t) + .23 * np.sin(6 * np.pi * hz * t)) * env * volume
        track[at:at + count, 0] += sig * (1 - .3 * pan)
        track[at:at + count, 1] += sig * (1 + .3 * pan)

    beat = 60 / bpm
    melody = [62, 65, 69, 72, 69, 65, 74, 72, 65, 69, 72, 77, 74, 72, 69, 65]
    for b in range(int(DURATION / beat)):
        at = b * beat
        note(at, beat * .8, [38, 34, 41, 36][(b // 8) % 4], .12)
        for k in range(2):
            note(at + k * beat / 2, beat * .35, melody[(2 * b + k) % 16], .035, k - .5)
        count = min(int(.15 * sr), len(track) - int(at * sr))
        t = np.arange(count) / sr
        kick = np.sin(2 * np.pi * (47 * t + 1.3 * (1 - np.exp(-t * 27)))) * np.exp(-t * 28) * .17
        track[int(at * sr):int(at * sr) + count] += kick[:, None]
        if b % 2:
            noise = rng.uniform(-1, 1, count) * np.exp(-t * 35) * .045
            track[int(at * sr):int(at * sr) + count] += noise[:, None]
    for at in [.4, .95, 1.5]:
        note(at, .14, 76, .16)
    note(GO, .5, 88, .16)
    for at in scene.stage_times[1:-1]:
        note(at, .08, 81, .06)
    for i, n in enumerate([62, 65, 69, 74, 77]):
        note(freeze_at + i * .1, .9, n, .13)
    start, end = int(GO * sr), int(freeze_at * sr)
    t = np.arange(end - start, dtype=np.float32) / sr
    ramp = np.minimum(t * 2, 1) * np.minimum((len(t) / sr - t) * 5, 1)
    motor = (np.sin(2 * np.pi * (85 * t + .8 * np.sin(t * 9))) + .3 * np.sin(2 * np.pi * 170 * t)) * ramp * .018
    track[start:end] += motor[:, None]
    fade = np.minimum(np.arange(len(track)) / sr * 3, 1) * np.minimum((len(track) - np.arange(len(track))) / sr, 1)
    track *= fade[:, None]
    with wave.open(str(path), "wb") as f:
        f.setnchannels(2)
        f.setsampwidth(2)
        f.setframerate(sr)
        f.writeframes((np.tanh(track) * 28000).astype("<i2").tobytes())
