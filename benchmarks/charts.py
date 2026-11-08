#!/usr/bin/env python3
"""Draw the README's charts (SVG, light and dark) from the results in results/.

    python3 charts.py        # → charts/*.svg, and each chart's alt text copied into ../README.md

Standard library only. Each chart is drawn at the width GitHub shows it, so it reads without
zooming. json-cursor is in the accent colour and every other parser in grey, and each bar is
labelled, so identity never depends on colour alone.
"""

import csv
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
RESULTS = os.path.join(HERE, "results")
CHARTS = os.path.join(HERE, "charts")
README = os.path.join(HERE, "..", "README.md")
# Where the README finds the charts: relative, so they show on every branch.
RAW = "benchmarks/charts/"

# The same palette as the other FusionVault crates' charts.
THEMES = {
    "light": dict(surface="#fcfcfb", border="#e1e0d9", ink="#0b0b0b", ink2="#52514e", muted="#6f6d68",
                  grid="#e9e8e2", axis="#c3c2b7", accent="#2a78d6", other="#898781",
                  good="#0ca30c", bad="#d03b3b"),
    "dark": dict(surface="#1a1a19", border="#2c2c2a", ink="#ffffff", ink2="#c3c2b7", muted="#9b9a93",
                 grid="#2c2c2a", axis="#383835", accent="#3987e5", other="#75746d",
                 good="#0ca30c", bad="#d03b3b"),
}
FONT = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
WIDTH = 760
OURS = "json-cursor"
# The parsers a chart shows, in a fixed order. serde_json Value builds a tree, so it is left to
# the tables.
PARSERS = [OURS, "jiter", "serde_json", "simd-json", "sonic-rs"]
SHAPES = {"bids": "Bids", "events": "Events", "features": "GeoJSON"}
# Within 3 % is a tie: closer than the spread between builds.
TIE = 0.03


# ── Reading the results ────────────────────────────────────────────────────────────────────────

def read_csv(name):
    with open(os.path.join(RESULTS, name)) as f:
        return list(csv.DictReader(f))


def speed(job):
    """[(shape, bytes, [(parser, ns)])] for one job, the parsers in PARSERS' order."""
    rows = read_csv("speed.csv")
    out = []
    for shape in SHAPES:
        times = {r["parser"]: float(r["ns_per_message"]) for r in rows if (r["shape"], r["job"]) == (shape, job)}
        size = next(int(r["bytes_per_message"]) for r in rows if r["shape"] == shape)
        out.append((shape, size, [(p, times[p]) for p in PARSERS]))
    return out


def verdict(bars, shape):
    """How json-cursor stands against the fastest other parser: words for the panel, and for the alt text."""
    ours = dict(bars)[OURS]
    rival, best = min(((p, v) for p, v in bars if p != OURS), key=lambda pv: pv[1])
    lead = 1 - ours / best
    if lead > TIE:
        return f"fastest, {lead:.0%} ahead of {rival}", f"fastest on {shape} ({lead:.0%} ahead of {rival})"
    if lead >= -TIE:
        return f"level with {rival}", f"level with {rival} on {shape}"
    return f"{-lead:.0%} behind {rival}", f"{-lead:.0%} behind {rival} on {shape}"


# ── Formatting ─────────────────────────────────────────────────────────────────────────────────

def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def text_w(s, size):
    return len(s) * size * 0.56  # a fair estimate for a system sans


def ns(v):
    return f"{v / 1000:.2f} µs" if v >= 1000 else f"{v:.0f} ns"


def mix(fg, bg, alpha):
    """`fg` at `alpha` over `bg`, as a solid colour (some renderers ignore SVG opacity)."""
    f, b = (tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)) for c in (fg, bg))
    return "#" + "".join(f"{round(x * alpha + y * (1 - alpha)):02x}" for x, y in zip(f, b))


# ── SVG pieces ─────────────────────────────────────────────────────────────────────────────────

class Svg:
    def __init__(self, theme):
        self.t, self.parts, self.h = THEMES[theme], [], 0

    def add(self, s):
        self.parts.append(s)

    def text(self, x, y, s, size=15, fill="ink2", weight=400, anchor="start"):
        self.add(f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
                 f'fill="{self.t[fill]}" text-anchor="{anchor}">{esc(s)}</text>')

    def header(self, title, subtitle):
        self.text(28, 40, title, 21, "ink", 600)
        self.text(28, 66, subtitle, 15, "ink2")
        return 92

    def bar(self, x0, y, x1, h, fill):
        """A horizontal bar: square at the baseline, 4 px rounded at the data end."""
        r = min(4.0, (x1 - x0) / 2, h / 2)
        self.add(f'<path d="M{x0:.1f},{y:.1f} H{x1 - r:.1f} A{r},{r} 0 0 1 {x1:.1f},{y + r:.1f} '
                 f'V{y + h - r:.1f} A{r},{r} 0 0 1 {x1 - r:.1f},{y + h:.1f} H{x0:.1f} Z" fill="{self.t[fill]}"/>')

    def status(self, cx, cy, ok):
        """A status icon: a filled circle with a check or a cross (never colour alone)."""
        self.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="8" fill="{self.t["good" if ok else "bad"]}"/>')
        if ok:
            d = f"M{cx - 3.6:.1f},{cy + 0.2:.1f} L{cx - 1:.1f},{cy + 2.9:.1f} L{cx + 3.8:.1f},{cy - 2.9:.1f}"
        else:
            d = (f"M{cx - 2.9:.1f},{cy - 2.9:.1f} L{cx + 2.9:.1f},{cy + 2.9:.1f} "
                 f"M{cx + 2.9:.1f},{cy - 2.9:.1f} L{cx - 2.9:.1f},{cy + 2.9:.1f}")
        self.add(f'<path d="{d}" stroke="#ffffff" stroke-width="2" fill="none" stroke-linecap="round" '
                 f'stroke-linejoin="round"/>')

    def footnote(self, y, lines):
        for line in lines:
            self.text(28, y, line, 13.5, "muted")
            y += 20
        self.h = int(y + 2)

    def save(self, name, theme, alt):
        t = self.t
        body = "\n".join(self.parts)
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{self.h}" '
               f'viewBox="0 0 {WIDTH} {self.h}" font-family="{FONT}" role="img" aria-label="{esc(alt)}">\n'
               f'<title>{esc(alt)}</title>\n'
               f'<rect x="0.5" y="0.5" width="{WIDTH - 1}" height="{self.h - 1}" rx="10" fill="{t["surface"]}" '
               f'stroke="{t["border"]}"/>\n{body}\n</svg>\n')
        with open(os.path.join(CHARTS, f"{name}-{theme}.svg"), "w") as f:
            f.write(svg)


# ── The charts ─────────────────────────────────────────────────────────────────────────────────

def speed_chart(name, title, panels, footnote):
    """One panel per shape, stacked, each with one bar per parser and its own scale."""
    row_h, bar_h, label_w = 30, 18, 112
    x0, x1 = 28 + label_w, WIDTH - 28 - 74
    for theme in THEMES:
        s = Svg(theme)
        y = s.header(title, "Time per message. Shorter is faster.")
        for shape, size, bars in panels:
            s.text(28, y + 22, f"{SHAPES[shape]}, {size:,} bytes", 17, "ink", 600)
            words, _ = verdict(bars, SHAPES[shape])
            s.text(WIDTH - 28, y + 22, words, 15, "ink2", 400, "end")
            top = y + 38
            vmax = max(v for _, v in bars)
            for i, (parser, v) in enumerate(bars):
                ours = parser == OURS
                cy = top + i * row_h + row_h / 2
                s.text(28, cy + 5, parser, 15, "ink" if ours else "ink2", 600 if ours else 400)
                x = x0 + v / vmax * (x1 - x0)
                s.bar(x0, cy - bar_h / 2, x, bar_h, "accent" if ours else "other")
                s.text(x + 8, cy + 5, ns(v), 15, "ink", 600 if ours else 400)
            s.add(f'<line x1="{x0}" y1="{top}" x2="{x0}" y2="{top + row_h * len(bars)}" stroke="{s.t["axis"]}"/>')
            y = top + row_h * len(bars) + 22
        s.footnote(y + 6, footnote)
        verdicts = [verdict(bars, SHAPES[shape].lower().replace("geojson", "GeoJSON"))[1] for shape, _, bars in panels]
        s.save(name, theme, f"{title}: json-cursor is " + ", ".join(verdicts[:-1]) + " and " + verdicts[-1])


def strict_chart(rows):
    """Parsers across, cases down, each cell RFC 8259's answer or not, with a word."""
    label_w = 236
    col_w = (WIDTH - 28 - 28 - label_w) / len(PARSERS)
    cx0 = 28 + label_w
    for theme in THEMES:
        s = Svg(theme)
        y = s.header("Strictness", "A check is RFC 8259's answer. A cross is not, or a crash.")
        top = y + 4
        # json-cursor's column carries the emphasis wash
        wash = mix(s.t["accent"], s.t["surface"], 0.10 if theme == "light" else 0.18)
        s.add(f'<rect x="{cx0:.1f}" y="{top:.1f}" width="{col_w:.1f}" height="{38 + 40 * len(rows):.1f}" '
              f'rx="6" fill="{wash}"/>')
        for j, p in enumerate(PARSERS):
            s.text(cx0 + col_w * (j + 0.5), top + 25, p, 15, "ink", 600 if p == OURS else 500, "middle")
        for i, (label, cells) in enumerate(rows):
            ry = top + 38 + 40 * i
            s.add(f'<line x1="28" y1="{ry:.1f}" x2="{WIDTH - 28}" y2="{ry:.1f}" stroke="{s.t["grid"]}"/>')
            s.text(28, ry + 25, label, 15, "ink")
            for j, (ok, word) in enumerate(cells):
                cx = cx0 + col_w * (j + 0.5)
                w = 22 + text_w(word, 14)
                s.status(cx - w / 2 + 8, ry + 20, ok)
                s.text(cx - w / 2 + 22, ry + 25, word, 14, "ink2")
        s.footnote(top + 38 + 40 * len(rows) + 28,
                   ["All five give RFC 8259's answer on the other 15 cases: commas, numbers, escapes,",
                    "control characters, trailing bytes and more."])
        cases = read_csv("strict.csv")
        wrong = {p: sum(1 for r in cases if not right(r, p)) for p in PARSERS}
        alt = (f"Strictness: json-cursor gives RFC 8259's answer on all {len(cases)} cases; "
               + "; ".join(f"{p} does not on {n}" for p, n in wrong.items() if n))
        s.save("strict", theme, alt)


def right(row, parser):
    """Whether a parser gave RFC 8259's answer: where the RFC leaves it open, anything but a crash."""
    return {"accept": "accepts", "refuse": "refuses"}.get(row["rfc_8259"], row[parser]) == row[parser] != "crashes"


def strict_rows():
    """The cases where a charted parser parts from RFC 8259, the five bad UTF-8 cases as one."""
    rows = read_csv("strict.csv")
    def cells(r):
        return [(right(r, p), r[p]) for p in PARSERS]
    utf8 = [r for r in rows if "UTF-8" in r["case"]]
    for r in utf8:  # the five forms of bad UTF-8 get one row, so they must agree
        assert cells(r) == cells(utf8[0]), r["case"]
    by_case = {r["case"]: r for r in rows}
    return [
        ("Bad UTF-8 in a string", cells(utf8[0])),
        ("1e400, past a double", cells(by_case["1e400 past a double's range"])),
        ("A 30-digit integer", cells(by_case["a 30-digit integer"])),
        ("A million nested arrays", cells(by_case["a million nested arrays under one key"])),
    ]


def sync_alt_texts():
    """Copy each chart's alt text into the README, so the two never drift apart."""
    with open(README) as f:
        text = f.read()

    def alt(m):
        with open(os.path.join(CHARTS, m.group(1) + "-light.svg")) as f:
            label = re.search(r'aria-label="([^"]*)"', f.read()).group(1)
        return f'<img alt="{label}" src="{RAW}{m.group(1)}-light.svg">'

    with open(README, "w") as f:
        f.write(re.sub(r'<img alt="[^"]*" src="' + re.escape(RAW) + r'([a-z-]+)-light\.svg">', alt, text))


def main():
    os.makedirs(CHARTS, exist_ok=True)
    speed_chart("read", "Reading every value", speed("read every value"),
                ["Each parser visits every key and value, as a decoder does."])
    speed_chart("validate", "Validating", speed("validate"),
                ["jiter and serde_json skip the UTF-8 check inside strings when they only validate.",
                 "json-cursor, simd-json and sonic-rs make it."])
    strict_chart(strict_rows())
    sync_alt_texts()


if __name__ == "__main__":
    main()
