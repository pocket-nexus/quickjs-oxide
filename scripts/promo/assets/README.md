# Relay artwork and type

The rally-car construction in `../relay.py` is adapted from
[MoeForge/moejs's race kit](https://github.com/MoeForge/moejs/blob/3035a4d642967b2eddeee8164ef3010ec85fc401/bench/race/pix.py),
at commit `3035a4d642967b2eddeee8164ef3010ec85fc401`. The car badges and
colors identify the engines in this comparison. The upstream code is
licensed under [Apache 2.0](MoeJS-Apache-2.0.txt). No frames or audio are
extracted from the reference video for use in the film.

`pixel-font.json` is a small raster glyph atlas made from Fusion Pixel 12px
Proportional, simplified Chinese, release
[2026.11.10](https://github.com/TakWolf/fusion-pixel-font/releases/tag/2026.11.10).
It stores each glyph's bitmap rows, offsets and advance at its native 12px
size. The renderer uses integer scaling only. The original TTF's SHA-256 is
`7e93fedda2c55bb37badb2262aaf43b08e3cb5976a0e3de2d90986ce59fe902e`;
the atlas also records the release archive's hash. No font is installed.

Fusion Pixel is Copyright (c) 2022 TakWolf, under the SIL Open Font License
1.1. Its notices and the source fonts' notices accompany the atlas:

- [Fusion Pixel](Fusion-OFL.txt)
- [Ark Pixel](Ark-OFL.txt)
- [Cubic 11](Cubic-OFL.txt)
- [Galmuri](Galmuri-LICENSE.txt)

The scene composition, camera integration and synthesized soundtrack are
specific to this relay. The measurement data remain separate from the art.
