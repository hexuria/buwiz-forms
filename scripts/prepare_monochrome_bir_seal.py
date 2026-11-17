#!/usr/bin/env python3
"""Create reviewed monochrome BIR seal candidates without rasterization.

The input is the exact Wikimedia Commons SVG pinned by SHA-256 below.  The
conversion changes only hexadecimal color tokens.  Grayscale mode uses integer
sRGB relative luminance.  Binary mode applies one of three documented,
non-reference-tuned thresholds to that same luminance.  Geometry, path data,
transforms, element order, and text remain untouched so review can compare the
vector source and derivatives.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


SOURCE_SHA256 = "9e1c158416b396bfb2d3b7820cf56ace8ed080aff53651dc880606a3e75e7aa7"
COLOR_TOKEN = re.compile(r"#(?:[0-9A-Fa-f]{6}|[0-9A-Fa-f]{3})\b")
GRAYSCALE_NOTE = (
    "<!-- Deterministic monochrome derivative: sRGB relative luminance "
    "(2126R + 7152G + 722B) / 10000; vector geometry unchanged. -->"
)
REVIEWED_BINARY_THRESHOLDS = (128, 192, 255)


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def path_data_digest(svg_text: str) -> tuple[int, str]:
    root = ET.fromstring(svg_text)
    path_data = [
        element.attrib.get("d", "")
        for element in root.iter()
        if element.tag.rsplit("}", 1)[-1] == "path"
    ]
    payload = "\0".join(path_data).encode("utf-8")
    return len(path_data), sha256(payload)


def rgb_and_luminance(match: re.Match[str]) -> tuple[int, int, int, int]:
    digits = match.group(0)[1:]
    if len(digits) == 3:
        digits = "".join(character * 2 for character in digits)
    red, green, blue = (int(digits[index : index + 2], 16) for index in (0, 2, 4))
    gray = (2126 * red + 7152 * green + 722 * blue + 5000) // 10000
    return red, green, blue, gray


def grayscale_token(match: re.Match[str]) -> str:
    _, _, _, gray = rgb_and_luminance(match)
    return f"#{gray:02x}{gray:02x}{gray:02x}"


def binary_token(match: re.Match[str], threshold: int) -> str:
    _, _, _, luminance = rgb_and_luminance(match)
    return "#ffffff" if luminance >= threshold else "#000000"


def transform(
    source: bytes,
    *,
    mode: str = "grayscale",
    threshold: int | None = None,
) -> bytes:
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("source does not match the pinned Wikimedia Commons SVG")

    if mode == "grayscale":
        if threshold is not None:
            raise ValueError("grayscale mode does not accept a binary threshold")
        replacement = grayscale_token
        generator_note = GRAYSCALE_NOTE
    elif mode == "binary":
        if threshold not in REVIEWED_BINARY_THRESHOLDS:
            raise ValueError(
                "binary mode requires a reviewed threshold: "
                + ", ".join(str(value) for value in REVIEWED_BINARY_THRESHOLDS)
            )
        replacement = lambda match: binary_token(match, threshold)
        generator_note = (
            "<!-- Deterministic binary derivative: integer sRGB relative "
            f"luminance >= {threshold} maps to white, otherwise black; "
            "vector geometry unchanged. -->"
        )
    else:
        raise ValueError(f"unsupported conversion mode: {mode}")

    source_text = source.decode("utf-8")
    root = ET.fromstring(source_text)
    if any(element.tag.rsplit("}", 1)[-1] == "image" for element in root.iter()):
        raise ValueError("source unexpectedly contains a raster image element")

    derivative = COLOR_TOKEN.sub(replacement, source_text)
    marker = "<!-- Created with Inkscape (http://www.inkscape.org/) -->"
    if marker not in derivative:
        raise ValueError("source no longer contains the expected generator marker")
    derivative = derivative.replace(marker, f"{marker}\n{generator_note}", 1)

    source_paths = path_data_digest(source_text)
    derivative_paths = path_data_digest(derivative)
    if derivative_paths != source_paths:
        raise ValueError("monochrome conversion changed vector path data")

    remaining_colors = set(COLOR_TOKEN.findall(derivative))
    if mode == "grayscale":
        if any(
            not (token[1:3] == token[3:5] == token[5:7])
            for token in remaining_colors
        ):
            raise ValueError("monochrome conversion left a non-gray color token")
    elif remaining_colors - {"#000000", "#ffffff"}:
        raise ValueError("binary conversion left a non-binary color token")

    return derivative.encode("utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--mode",
        choices=("grayscale", "binary"),
        default="grayscale",
    )
    parser.add_argument(
        "--threshold",
        type=int,
        choices=REVIEWED_BINARY_THRESHOLDS,
    )
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()

    try:
        derivative = transform(
            arguments.source.read_bytes(),
            mode=arguments.mode,
            threshold=arguments.threshold,
        )
    except (OSError, UnicodeError, ValueError, ET.ParseError) as error:
        print(f"BIR seal preparation failed: {error}", file=sys.stderr)
        return 1

    if arguments.check:
        try:
            current = arguments.output.read_bytes()
        except OSError as error:
            print(f"BIR seal check failed: {error}", file=sys.stderr)
            return 1
        if current != derivative:
            print("BIR seal derivative is stale", file=sys.stderr)
            return 1
    else:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_bytes(derivative)

    count, path_digest = path_data_digest(derivative.decode("utf-8"))
    print(
        f"source_sha256={SOURCE_SHA256} "
        f"derivative_sha256={sha256(derivative)} "
        f"mode={arguments.mode} "
        f"threshold={arguments.threshold if arguments.threshold is not None else 'none'} "
        f"paths={count} path_data_sha256={path_digest}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
