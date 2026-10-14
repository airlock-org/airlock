# Airlock logo

`logo.svg` is an unchanged copy of the logo supplied from
`/home/user/dev/airlock-org/assets/logo.svg`. The app retains its octagonal
frame, orange doorway, arrows, gradients and transparent background.

`logo.png` is a 256 × 256 rendering with the original aspect ratio and a small
transparent margin. It is embedded in the binary for the header and native
window icon; running or building the app needs no external asset directory.

To regenerate it from the repository root with Inkscape installed:

```sh
inkscape apps/airlock-ctrl/assets/logo.svg \
  --export-area=-5:0:657:662 --export-width=256 --export-height=256 \
  --export-filename=apps/airlock-ctrl/assets/logo.png
```
