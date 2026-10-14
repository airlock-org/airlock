"""Create native TTF instances from the landing page's bundled Inter font."""

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

DIRECTORY = Path(__file__).resolve().parent

for weight, name in [
    (400, "Inter-Regular.ttf"),
    (500, "Inter-Medium.ttf"),
    (600, "Inter-SemiBold.ttf"),
]:
    font = TTFont(DIRECTORY / "InterVariable.woff2", recalcTimestamp=False)
    font.flavor = None
    font = instantiateVariableFont(font, {"wght": weight, "opsz": 14}, inplace=True)
    font.save(DIRECTORY / name)
