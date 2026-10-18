#!/usr/bin/env python3
"""Generates the Blaze logo family: a Rust-style 8-tooth cog with a knocked-out glyph per tool.
Construction follows the svg-design guidance: geometric, one compound evenodd path, <=2 colours,
a currentColor variant, and a gradient variant. Output: brand/*.svg (copied to site/public/logos)."""
import math, os

HERE = os.path.dirname(os.path.abspath(__file__))
C = 32.0

def cog(teeth=8, r_root=22.5, r_tip=28.5, base=13.5, tip=8.0):
    pts = []
    d = ""
    for i in range(teeth):
        a = i * 360 / teeth - 90
        def p(off, r):
            t = math.radians(a + off)
            return C + r * math.cos(t), C + r * math.sin(t)
        pl, tl, tr, pr = p(-base, r_root), p(-tip, r_tip), p(tip, r_tip), p(base, r_root)
        d += ("M" if i == 0 else f"A{r_root} {r_root} 0 0 1") + f"{pl[0]:.2f} {pl[1]:.2f}"
        d += f"L{tl[0]:.2f} {tl[1]:.2f}L{tr[0]:.2f} {tr[1]:.2f}L{pr[0]:.2f} {pr[1]:.2f}"
        nxt = p(360 / teeth - base, r_root)
        if i == teeth - 1:
            first = (C + r_root * math.cos(math.radians(-90 - base)), C + r_root * math.sin(math.radians(-90 - base)))
            d += f"A{r_root} {r_root} 0 0 1{first[0]:.2f} {first[1]:.2f}Z"
    return d

def circle(cx, cy, r):
    return f"M{cx - r:.2f} {cy:.2f}A{r} {r} 0 1 0 {cx + r:.2f} {cy:.2f}A{r} {r} 0 1 0 {cx - r:.2f} {cy:.2f}Z"

def ring(hole=16.5, rivets=8, rivet_r=1.35, rivet_at=19.6):
    """Rust-style cog: toothed ring, round hub opening, small rivet holes between the teeth."""
    d = cog() + circle(C, C, hole)
    for i in range(rivets):
        t = math.radians(i * 360 / rivets - 90 + 360 / rivets / 2)
        d += circle(C + rivet_at * math.cos(t), C + rivet_at * math.sin(t), rivet_r)
    return d

GLYPH_SCALE = 0.8  # glyph sits inside the hub opening, like the R in the Rust logo
def glyph(name):
    return f'<path fill="FILL" transform="translate(32 32) scale({GLYPH_SCALE}) translate(-32 -32)" d="{GLYPHS[name]}"/>'

# centred glyphs inside the hub (radius ~15), each a single closed subpath
GLYPHS = {
    # flame
    "blaze": "M32 15.5C33.2 22.5 40.5 25.5 40.5 34.5C40.5 40.5 36.8 45.5 32 45.5C27.2 45.5 23.5 40.5 23.5 34.5C23.5 30.8 25.4 28 27.6 25.8C28.2 28.6 29.8 30 31 30C29.6 25 30.2 19.6 32 15.5Z",
    # glowing ember / four-point spark
    "ember": "M32 15C33.3 25 39 30.7 49 32C39 33.3 33.3 39 32 49C30.7 39 25 33.3 15 32C25 30.7 30.7 25 32 15Z",
    # anvil
    "forge": "M15.5 25.5H44.5V28C44.5 30.4 41.8 32 38 32V34.5H40.5V38.5H42.5V43H21.5V38.5H23.5V34.5H26V32C22 32 18.6 29.6 15.5 25.5Z",
    # lightning bolt
    "spark": "M35.5 15.5L22.5 34H30.5L28.5 48.5L41.5 29.5H33.5L35.5 15.5Z",
    # shield
    "ward": "M32 15.5L43 19.8V31C43 38.2 38.2 43.4 32 47C25.8 43.4 21 38.2 21 31V19.8Z",
}
GRAD = '<defs><linearGradient id="g" x1="8" y1="6" x2="56" y2="58" gradientUnits="userSpaceOnUse"><stop stop-color="#ff7a3d"/><stop offset="1" stop-color="#d4401b"/></linearGradient></defs>'

def body(name, fill):
    return (f'<path fill-rule="evenodd" fill="{fill}" d="{ring()}"/>' + glyph(name).replace("FILL", fill))

def svg(name, fill, extra_defs=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" role="img" aria-label="{name}">'
            f'{extra_defs}{body(name, fill)}</svg>')

for n in GLYPHS:
    open(os.path.join(HERE, f"{n}.svg"), "w").write(svg(n, "url(#g)", GRAD))
    open(os.path.join(HERE, f"{n}-mono.svg"), "w").write(svg(n, "currentColor"))
fav = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">{GRAD}<rect width="64" height="64" rx="14" fill="#07080a"/>'
       f'<g transform="translate(6 6) scale(.8125)">{body("blaze", "url(#g)")}</g></svg>')
open(os.path.join(HERE, "favicon.svg"), "w").write(fav)
lock = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 64">{GRAD}{body("blaze", "url(#g)")}'
        f'<text x="74" y="43" font-family="Inter,system-ui,sans-serif" font-size="34" font-weight="700" letter-spacing="-1" fill="currentColor">blaze</text></svg>')
open(os.path.join(HERE, "lockup.svg"), "w").write(lock)
print("logos:", sorted(f for f in os.listdir(HERE) if f.endswith(".svg")))

# ring-only cog (no glyph) used as a rotating hero backdrop
open(os.path.join(HERE, "cog.svg"), "w").write(
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">{GRAD}<path fill-rule="evenodd" fill="url(#g)" d="{ring()}"/></svg>')
