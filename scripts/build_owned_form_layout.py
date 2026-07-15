#!/usr/bin/env python3
"""Build a review-only 2551Q owned-layout candidate from calibrated sources.

The generated JSON is an intermediate development artifact.  It deliberately
does not write into packages/form-specs: a human must review and simplify the
candidate before any geometry becomes part of the runtime renderer.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from collections import defaultdict
from dataclasses import dataclass
from decimal import Decimal, ROUND_HALF_UP
from pathlib import Path
from typing import Any, Mapping, Sequence


GENERATOR_NAME = "owned-form-layout-candidate"
GENERATOR_VERSION = "1.0.0"
EXPECTED_FORM_ID = "2551Qv2018"
EXPECTED_OFFICIAL_SOURCE_SHA256 = (
    "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24"
)
PAGE_WIDTH_PT = 612.0
PAGE_HEIGHT_PT = 936.0
PAGE_COUNT = 2

ROUNDING_TOLERANCE_PT = 0.1
SOURCE_BOUNDS_TOLERANCE_PT = 0.2
RULE_MAX_THICKNESS_PT = 1.2
RULE_MIN_LENGTH_PT = 2.0
RULE_AXIS_TOLERANCE_PT = 0.2
RULE_THICKNESS_TOLERANCE_PT = 0.2
RULE_MERGE_GAP_PT = 0.2
MIN_FILL_AREA_PT2 = 12.0
MIN_FILL_EDGE_PT = 1.5
BINDING_OVERLAP_TOLERANCE_PT = 0.2

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FORM_DIR = REPO_ROOT / "formtypes" / EXPECTED_FORM_ID
DEFAULT_OUTPUT = (
    REPO_ROOT / ".scratch" / "form-layouts" / "2551q-2018" / "candidate.json"
)


class LayoutError(ValueError):
    """Raised when calibrated source data cannot produce an unambiguous candidate."""


@dataclass(frozen=True)
class SourceDocuments:
    form_dir: Path
    structure_path: Path
    formtype_path: Path
    metadata_path: Path
    structure: Mapping[str, Any]
    formtype: Mapping[str, Any]
    metadata: Mapping[str, Any]
    hashes: Mapping[str, str]


@dataclass(frozen=True)
class RuleSegment:
    page: int
    orientation: str
    position: float
    start: float
    end: float
    thickness: float
    color: str


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_json_object(path: Path) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise LayoutError(f"missing required source file: {path}") from error
    except json.JSONDecodeError as error:
        raise LayoutError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, dict):
        raise LayoutError(f"{path} must contain a JSON object")
    return value


def load_sources(form_dir: Path) -> SourceDocuments:
    form_dir = form_dir.resolve()
    structure_path = form_dir / "form_structure.json"
    formtype_path = form_dir / "formtype.json"
    metadata_path = form_dir / "metadata.json"
    return SourceDocuments(
        form_dir=form_dir,
        structure_path=structure_path,
        formtype_path=formtype_path,
        metadata_path=metadata_path,
        structure=_read_json_object(structure_path),
        formtype=_read_json_object(formtype_path),
        metadata=_read_json_object(metadata_path),
        hashes={
            "form_structure.json": _sha256(structure_path),
            "formtype.json": _sha256(formtype_path),
            "metadata.json": _sha256(metadata_path),
        },
    )


def _number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise LayoutError(f"{label} must be a finite number")
    result = float(value)
    if not math.isfinite(result):
        raise LayoutError(f"{label} must be a finite number")
    return result


def _integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise LayoutError(f"{label} must be an integer")
    return value


def _round_pt(value: float) -> float:
    rounded = Decimal(str(value)).quantize(Decimal("0.1"), rounding=ROUND_HALF_UP)
    result = float(rounded)
    return 0.0 if result == -0.0 else result


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise LayoutError(f"{label} must be an array")
    return value


def _validate_page(page_value: Any, label: str) -> int:
    page = _integer(page_value, label)
    if page < 1 or page > PAGE_COUNT:
        raise LayoutError(f"{label} must be between 1 and {PAGE_COUNT}, got {page}")
    return page


def _validate_anchor(page: int, x: float, y: float, label: str) -> None:
    del page  # Page is validated separately; keeping it in the signature aids callers.
    tolerance = SOURCE_BOUNDS_TOLERANCE_PT
    if x < -tolerance or x > PAGE_WIDTH_PT + tolerance:
        raise LayoutError(f"{label} x={x} falls outside the {PAGE_WIDTH_PT}pt page")
    if y < -tolerance or y > PAGE_HEIGHT_PT + tolerance:
        raise LayoutError(f"{label} y={y} falls outside the {PAGE_HEIGHT_PT}pt page")


def _validate_box(
    page: int,
    x: float,
    y: float,
    width: float,
    height: float,
    label: str,
) -> None:
    _validate_anchor(page, x, y, label)
    if width <= 0.0 or height <= 0.0:
        raise LayoutError(f"{label} must have positive width and height")
    tolerance = SOURCE_BOUNDS_TOLERANCE_PT
    if x + width > PAGE_WIDTH_PT + tolerance:
        raise LayoutError(
            f"{label} right edge {x + width} exceeds the {PAGE_WIDTH_PT}pt page"
        )
    if y + height > PAGE_HEIGHT_PT + tolerance:
        raise LayoutError(
            f"{label} bottom edge {y + height} exceeds the {PAGE_HEIGHT_PT}pt page"
        )


def _validate_dimensions(documents: SourceDocuments) -> None:
    structure_dimensions = documents.structure.get("page_dimensions")
    if not isinstance(structure_dimensions, dict):
        raise LayoutError("form_structure.json page_dimensions must be an object")

    dimensions = (
        (
            "form_structure.json",
            structure_dimensions.get("width"),
            structure_dimensions.get("height"),
            structure_dimensions.get("count"),
        ),
        (
            "formtype.json",
            documents.formtype.get("page_width"),
            documents.formtype.get("page_height"),
            PAGE_COUNT,
        ),
        (
            "metadata.json",
            documents.metadata.get("page_width_pt"),
            documents.metadata.get("page_height_pt"),
            documents.metadata.get("page_count"),
        ),
    )
    for source_name, width_value, height_value, count_value in dimensions:
        width = _number(width_value, f"{source_name} page width")
        height = _number(height_value, f"{source_name} page height")
        count = _integer(count_value, f"{source_name} page count")
        if not math.isclose(width, PAGE_WIDTH_PT, abs_tol=0.001):
            raise LayoutError(
                f"{source_name} page width must be {PAGE_WIDTH_PT}pt, got {width}"
            )
        if not math.isclose(height, PAGE_HEIGHT_PT, abs_tol=0.001):
            raise LayoutError(
                f"{source_name} page height must be {PAGE_HEIGHT_PT}pt, got {height}"
            )
        if count != PAGE_COUNT:
            raise LayoutError(
                f"{source_name} page count must be {PAGE_COUNT}, got {count}"
            )


def validate_sources(documents: SourceDocuments) -> None:
    for source_name, document in (
        ("form_structure.json", documents.structure),
        ("formtype.json", documents.formtype),
        ("metadata.json", documents.metadata),
    ):
        form_id = document.get("form_id")
        if form_id != EXPECTED_FORM_ID:
            raise LayoutError(
                f"{source_name} form_id must be {EXPECTED_FORM_ID!r}, got {form_id!r}"
            )

    _validate_dimensions(documents)

    official_hash = documents.metadata.get("sha256")
    if not isinstance(official_hash, str):
        raise LayoutError("metadata.json sha256 must be a string")
    if official_hash.lower() != EXPECTED_OFFICIAL_SOURCE_SHA256:
        raise LayoutError(
            "official-source hash drift: expected "
            f"{EXPECTED_OFFICIAL_SOURCE_SHA256}, got {official_hash.lower()}"
        )
    official_url = documents.metadata.get("official_source")
    if not isinstance(official_url, str) or not official_url.startswith("https://"):
        raise LayoutError("metadata.json official_source must be an https URL")
    title = documents.metadata.get("title")
    if not isinstance(title, str) or not title.strip():
        raise LayoutError("metadata.json title must be a non-empty string")

    rectangles = _require_list(
        documents.structure.get("rectangles"), "form_structure.json rectangles"
    )
    text_blocks = _require_list(
        documents.structure.get("text_blocks"), "form_structure.json text_blocks"
    )
    fields = _require_list(documents.formtype.get("fields"), "formtype.json fields")

    for index, raw in enumerate(rectangles):
        label = f"rectangle[{index}]"
        if not isinstance(raw, dict):
            raise LayoutError(f"{label} must be an object")
        page = _validate_page(raw.get("page"), f"{label}.page")
        x = _number(raw.get("x"), f"{label}.x")
        y = _number(raw.get("y"), f"{label}.y")
        width = _number(raw.get("w"), f"{label}.w")
        height = _number(raw.get("h"), f"{label}.h")
        _validate_box(page, x, y, width, height, label)
        for paint_key in ("fill", "stroke"):
            paint = raw.get(paint_key)
            if paint is not None and not isinstance(paint, str):
                raise LayoutError(f"{label}.{paint_key} must be a string or null")
        stroke_width = raw.get("stroke_width")
        if stroke_width is not None:
            _number(stroke_width, f"{label}.stroke_width")

    for index, raw in enumerate(text_blocks):
        label = f"text_blocks[{index}]"
        if not isinstance(raw, dict):
            raise LayoutError(f"{label} must be an object")
        page = _validate_page(raw.get("page"), f"{label}.page")
        x = _number(raw.get("x"), f"{label}.x")
        baseline_y = _number(raw.get("y"), f"{label}.y")
        _validate_anchor(page, x, baseline_y, label)
        font_size = _number(raw.get("font_size"), f"{label}.font_size")
        if font_size <= 0.0:
            raise LayoutError(f"{label}.font_size must be positive")
        if not isinstance(raw.get("content"), str):
            raise LayoutError(f"{label}.content must be a string")
        if not isinstance(raw.get("font_name"), str):
            raise LayoutError(f"{label}.font_name must be a string")
        if not isinstance(raw.get("is_bold"), bool):
            raise LayoutError(f"{label}.is_bold must be a boolean")
        if not isinstance(raw.get("color"), str):
            raise LayoutError(f"{label}.color must be a string")

    _validated_field_boxes(fields)


def _paint_color(raw: Mapping[str, Any]) -> str | None:
    for key in ("fill", "stroke"):
        value = raw.get(key)
        if isinstance(value, str) and value:
            return value.lower()
    return None


def _rule_segment(raw: Mapping[str, Any]) -> RuleSegment | None:
    page = int(raw["page"])
    x = float(raw["x"])
    y = float(raw["y"])
    width = float(raw["w"])
    height = float(raw["h"])
    color = _paint_color(raw)
    if color is None:
        return None

    horizontal = height <= RULE_MAX_THICKNESS_PT and width >= RULE_MIN_LENGTH_PT
    vertical = width <= RULE_MAX_THICKNESS_PT and height >= RULE_MIN_LENGTH_PT
    if horizontal and vertical:
        horizontal = width >= height
        vertical = not horizontal
    if horizontal:
        return RuleSegment(
            page=page,
            orientation="horizontal",
            position=y + (height / 2.0),
            start=x,
            end=x + width,
            thickness=height,
            color=color,
        )
    if vertical:
        return RuleSegment(
            page=page,
            orientation="vertical",
            position=x + (width / 2.0),
            start=y,
            end=y + height,
            thickness=width,
            color=color,
        )
    return None


def _cluster_rule_lanes(segments: Sequence[RuleSegment]) -> list[list[RuleSegment]]:
    lanes: list[list[RuleSegment]] = []
    for segment in sorted(
        segments,
        key=lambda item: (item.position, item.thickness, item.start, item.end),
    ):
        candidates: list[tuple[float, int]] = []
        for lane_index, lane in enumerate(lanes):
            positions = [item.position for item in lane]
            thicknesses = [item.thickness for item in lane]
            if (
                max(max(positions), segment.position)
                - min(min(positions), segment.position)
                <= RULE_AXIS_TOLERANCE_PT
                and max(max(thicknesses), segment.thickness)
                - min(min(thicknesses), segment.thickness)
                <= RULE_THICKNESS_TOLERANCE_PT
            ):
                position_delta = abs(
                    segment.position - (sum(positions) / len(positions))
                )
                candidates.append((position_delta, lane_index))
        if candidates:
            _, closest_lane = min(candidates)
            lanes[closest_lane].append(segment)
        else:
            lanes.append([segment])
    return lanes


def merge_rules(rectangles: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[int, str, str], list[RuleSegment]] = defaultdict(list)
    for raw in rectangles:
        segment = _rule_segment(raw)
        if segment is not None:
            grouped[(segment.page, segment.orientation, segment.color)].append(segment)

    merged: list[dict[str, Any]] = []
    for (page, orientation, color), segments in sorted(grouped.items()):
        for lane in _cluster_rule_lanes(segments):
            lane = sorted(lane, key=lambda item: (item.start, item.end, item.position))
            current: list[RuleSegment] = []
            current_end = 0.0
            for segment in lane:
                if not current or segment.start <= current_end + RULE_MERGE_GAP_PT:
                    current.append(segment)
                    current_end = max(current_end, segment.end)
                    continue
                merged.append(_merged_rule(page, orientation, color, current))
                current = [segment]
                current_end = segment.end
            if current:
                merged.append(_merged_rule(page, orientation, color, current))

    merged.sort(
        key=lambda item: (
            item["page"],
            item["orientation"],
            item["position_pt"],
            item["start_pt"],
            item["end_pt"],
            item["color"],
        )
    )
    counters: dict[tuple[int, str], int] = defaultdict(int)
    for item in merged:
        counter_key = (item["page"], item["orientation"])
        counters[counter_key] += 1
        prefix = "h" if item["orientation"] == "horizontal" else "v"
        item["id"] = f"p{item['page']}-rule-{prefix}-{counters[counter_key]:04d}"
    return merged


def _merged_rule(
    page: int,
    orientation: str,
    color: str,
    segments: Sequence[RuleSegment],
) -> dict[str, Any]:
    total_length = sum(item.end - item.start for item in segments)
    weighted_position = sum(
        item.position * (item.end - item.start) for item in segments
    ) / total_length
    return {
        "page": page,
        "orientation": orientation,
        "position_pt": _round_pt(weighted_position),
        "start_pt": _round_pt(min(item.start for item in segments)),
        "end_pt": _round_pt(max(item.end for item in segments)),
        "thickness_pt": _round_pt(max(item.thickness for item in segments)),
        "color": color,
        "source_segments": len(segments),
    }


def _fill_role(color: str) -> str:
    normalized = color.lstrip("#")
    if len(normalized) != 6:
        return "color_region"
    try:
        red, green, blue = (
            int(normalized[index : index + 2], 16) for index in (0, 2, 4)
        )
    except ValueError:
        return "color_region"
    if max(red, green, blue) - min(red, green, blue) <= 4:
        luminance = (red + green + blue) / (3.0 * 255.0)
        if luminance >= 0.97:
            return "white_region"
        if luminance <= 0.08:
            return "solid_region"
        return "shaded_region"
    return "color_region"


def build_fill_candidates(
    rectangles: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    grouped: dict[tuple[Any, ...], int] = defaultdict(int)
    for raw in rectangles:
        if _rule_segment(raw) is not None:
            continue
        fill = raw.get("fill")
        if not isinstance(fill, str) or not fill:
            continue
        width = float(raw["w"])
        height = float(raw["h"])
        if (
            width * height < MIN_FILL_AREA_PT2
            or width < MIN_FILL_EDGE_PT
            or height < MIN_FILL_EDGE_PT
        ):
            continue
        stroke = raw.get("stroke")
        stroke_width = raw.get("stroke_width")
        key = (
            int(raw["page"]),
            _round_pt(float(raw["x"])),
            _round_pt(float(raw["y"])),
            _round_pt(width),
            _round_pt(height),
            fill.lower(),
            stroke.lower() if isinstance(stroke, str) else None,
            _round_pt(float(stroke_width))
            if isinstance(stroke_width, (int, float)) and not isinstance(stroke_width, bool)
            else None,
        )
        grouped[key] += 1

    candidates: list[dict[str, Any]] = []
    for key, source_rectangles in grouped.items():
        page, x, y, width, height, fill, stroke, stroke_width = key
        candidate: dict[str, Any] = {
            "page": page,
            "role_candidate": _fill_role(fill),
            "x_pt": x,
            "y_pt": y,
            "width_pt": width,
            "height_pt": height,
            "fill": fill,
            "area_pt2": _round_pt(width * height),
            "source_rectangles": source_rectangles,
        }
        if stroke is not None:
            candidate["stroke"] = stroke
        if stroke_width is not None:
            candidate["stroke_width_pt"] = stroke_width
        candidates.append(candidate)

    candidates.sort(
        key=lambda item: (
            item["page"],
            item["y_pt"],
            item["x_pt"],
            -item["area_pt2"],
            item["fill"],
        )
    )
    counters: dict[int, int] = defaultdict(int)
    for item in candidates:
        counters[item["page"]] += 1
        item["id"] = f"p{item['page']}-fill-{counters[item['page']]:04d}"
    return candidates


def _text_role(content: str, font_size: float, is_bold: bool) -> str:
    normalized = content.strip().lower()
    if normalized.startswith("part ") or normalized.startswith("schedule "):
        return "section_heading"
    if font_size >= 12.0 or (is_bold and font_size >= 10.0):
        return "heading"
    return "label"


def build_text_candidates(
    text_blocks: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    grouped: dict[tuple[Any, ...], int] = defaultdict(int)
    for raw in text_blocks:
        key = (
            int(raw["page"]),
            str(raw["content"]),
            _round_pt(float(raw["x"])),
            _round_pt(float(raw["y"])),
            _round_pt(float(raw["font_size"])),
            str(raw["font_name"]),
            bool(raw["is_bold"]),
            str(raw.get("color") or "#000000").lower(),
        )
        grouped[key] += 1

    candidates: list[dict[str, Any]] = []
    for key, source_blocks in grouped.items():
        page, content, x, baseline_y, font_size, font_name, is_bold, color = key
        candidates.append(
            {
                "page": page,
                "role_candidate": _text_role(content, font_size, is_bold),
                "content": content,
                "x_pt": x,
                "baseline_y_pt": baseline_y,
                "font_size_pt": font_size,
                "font_family": font_name,
                "font_weight": "bold" if is_bold else "normal",
                "color": color,
                "source_blocks": source_blocks,
            }
        )
    candidates.sort(
        key=lambda item: (
            item["page"],
            item["baseline_y_pt"],
            item["x_pt"],
            item["content"],
        )
    )
    counters: dict[int, int] = defaultdict(int)
    for item in candidates:
        counters[item["page"]] += 1
        item["id"] = f"p{item['page']}-text-{counters[item['page']]:04d}"
    return candidates


def _field_widget_box(
    raw: Mapping[str, Any], label: str
) -> tuple[int, float, float, float, float]:
    page = _validate_page(raw.get("page"), f"{label}.page")
    x = _number(raw.get("x"), f"{label}.x")
    y = _number(raw.get("y"), f"{label}.y")
    widget = raw.get("widget")
    if not isinstance(widget, dict):
        raise LayoutError(f"{label}.widget must be an object")
    fallback_size = raw.get("size")
    width_value = widget.get("width", fallback_size)
    height_value = widget.get("height", fallback_size)
    width = _number(width_value, f"{label}.widget.width")
    height = _number(height_value, f"{label}.widget.height")
    _validate_box(page, x, y, width, height, label)
    return page, x, y, width, height


def _validated_field_boxes(
    fields: Sequence[Any],
) -> list[tuple[int, Mapping[str, Any], tuple[int, float, float, float, float]]]:
    boxes: list[
        tuple[int, Mapping[str, Any], tuple[int, float, float, float, float]]
    ] = []
    for index, raw in enumerate(fields):
        label = f"fields[{index}]"
        if not isinstance(raw, dict):
            raise LayoutError(f"{label} must be an object")
        key = raw.get("key")
        kind = raw.get("kind")
        if not isinstance(key, str) or not key:
            raise LayoutError(f"{label}.key must be a non-empty string")
        if not isinstance(kind, str) or not kind:
            raise LayoutError(f"{label}.kind must be a non-empty string")
        if not isinstance(raw.get("optional"), bool):
            raise LayoutError(f"{label}.optional must be a boolean")
        widget = raw.get("widget")
        if not isinstance(widget, dict) or not isinstance(widget.get("type"), str):
            raise LayoutError(f"{label}.widget.type must be a string")
        boxes.append((index, raw, _field_widget_box(raw, label)))

    for left_index in range(len(boxes)):
        left_source_index, left, left_box = boxes[left_index]
        left_page, left_x, left_y, left_width, left_height = left_box
        for right_source_index, right, right_box in boxes[left_index + 1 :]:
            right_page, right_x, right_y, right_width, right_height = right_box
            if left_page != right_page:
                continue
            overlap_width = min(
                left_x + left_width, right_x + right_width
            ) - max(left_x, right_x)
            overlap_height = min(
                left_y + left_height, right_y + right_height
            ) - max(left_y, right_y)
            if (
                overlap_width > BINDING_OVERLAP_TOLERANCE_PT
                and overlap_height > BINDING_OVERLAP_TOLERANCE_PT
            ):
                raise LayoutError(
                    "conflicting dynamic binding overlap on page "
                    f"{left_page}: fields[{left_source_index}] {left['key']!r} and "
                    f"fields[{right_source_index}] {right['key']!r} overlap by "
                    f"{overlap_width:.3f} x {overlap_height:.3f}pt"
                )
    return boxes


def _normalized_widget(widget: Mapping[str, Any]) -> dict[str, Any]:
    normalized: dict[str, Any] = {}
    for key in sorted(widget):
        value = widget[key]
        output_key = {
            "width": "width_pt",
            "height": "height_pt",
            "font_size": "font_size_pt",
        }.get(key, key)
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            normalized[output_key] = _round_pt(float(value))
        else:
            normalized[output_key] = value
    return normalized


def _fragment_payload(
    raw: Mapping[str, Any],
    role: str,
    order: int,
    repeat_of_role: str | None = None,
) -> dict[str, Any]:
    widget = raw["widget"]
    payload: dict[str, Any] = {
        "order": order,
        "role": role,
        "x_pt": _round_pt(float(raw["x"])),
        "y_pt": _round_pt(float(raw["y"])),
        "widget": _normalized_widget(widget),
    }
    if repeat_of_role is not None:
        payload["repeat_of_role"] = repeat_of_role
    mappings = {
        "char_count": "character_count",
        "direction": "direction",
        "int_cells": "integer_cells",
        "dec_x": "decimal_anchor_x_pt",
        "cell_w": "cell_width_pt",
        "size": "size_pt",
    }
    for source_key, output_key in mappings.items():
        value = raw.get(source_key)
        if value is None:
            continue
        if source_key in {"dec_x", "cell_w", "size"}:
            payload[output_key] = _round_pt(float(value))
        else:
            payload[output_key] = value
    return payload


def _decimal_base_role(raw: Mapping[str, Any]) -> str:
    decimal_anchor = raw.get("dec_x")
    if decimal_anchor is None:
        raise LayoutError(
            f"decimal binding {raw['key']!r} has no dec_x and cannot be ordered"
        )
    anchor = _number(decimal_anchor, f"decimal binding {raw['key']!r}.dec_x")
    widget = raw["widget"]
    width = _number(widget.get("width", raw.get("size")), "decimal widget width")
    x = float(raw["x"])
    right = x + width
    if right <= anchor + BINDING_OVERLAP_TOLERANCE_PT:
        return "integer"
    if x >= anchor - BINDING_OVERLAP_TOLERANCE_PT:
        return "decimal"
    raise LayoutError(
        f"decimal binding {raw['key']!r} fragment spans its {anchor}pt decimal anchor"
    )


def build_dynamic_bindings(fields: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    _validated_field_boxes(fields)
    grouped: dict[tuple[str, int], list[Mapping[str, Any]]] = defaultdict(list)
    for raw in fields:
        grouped[(str(raw["key"]), int(raw["page"]))].append(raw)

    bindings: list[dict[str, Any]] = []
    for (field_key, page), fragments in sorted(
        grouped.items(), key=lambda item: (item[0][1], item[0][0])
    ):
        kinds = {str(fragment["kind"]) for fragment in fragments}
        if len(kinds) != 1:
            raise LayoutError(
                f"binding {field_key!r} on page {page} mixes kinds: {sorted(kinds)}"
            )
        optional_values = {bool(fragment.get("optional", False)) for fragment in fragments}
        if len(optional_values) != 1:
            raise LayoutError(
                f"binding {field_key!r} on page {page} has conflicting optional flags"
            )
        kind = next(iter(kinds))

        if kind == "dec":
            ordered = sorted(fragments, key=lambda item: (float(item["x"]), float(item["y"])))
            base_roles = [_decimal_base_role(fragment) for fragment in ordered]
        else:
            ordered = sorted(fragments, key=lambda item: (float(item["y"]), float(item["x"])))
            if len(ordered) == 1:
                base_roles = ["value"]
            elif kind == "char":
                base_roles = ["primary"] + ["continuation"] * (len(ordered) - 1)
            else:
                base_roles = ["primary"] + ["repeat"] * (len(ordered) - 1)

        seen_roles: dict[str, int] = defaultdict(int)
        normalized_fragments: list[dict[str, Any]] = []
        for order, (fragment, base_role) in enumerate(zip(ordered, base_roles, strict=True)):
            seen_roles[base_role] += 1
            if kind == "dec" and seen_roles[base_role] > 1:
                normalized_fragments.append(
                    _fragment_payload(
                        fragment,
                        role="repeat",
                        order=order,
                        repeat_of_role=base_role,
                    )
                )
            else:
                normalized_fragments.append(
                    _fragment_payload(fragment, role=base_role, order=order)
                )

        bindings.append(
            {
                "page": page,
                "field_key": field_key,
                "kind": kind,
                "optional": next(iter(optional_values)),
                "fragments": normalized_fragments,
            }
        )

    counters: dict[int, int] = defaultdict(int)
    for binding in bindings:
        counters[binding["page"]] += 1
        binding["id"] = f"p{binding['page']}-binding-{counters[binding['page']]:03d}"
    return bindings


def build_candidate(form_dir: Path = DEFAULT_FORM_DIR) -> dict[str, Any]:
    documents = load_sources(form_dir)
    validate_sources(documents)
    rectangles = _require_list(
        documents.structure["rectangles"], "form_structure.json rectangles"
    )
    text_blocks = _require_list(
        documents.structure["text_blocks"], "form_structure.json text_blocks"
    )
    fields = _require_list(documents.formtype["fields"], "formtype.json fields")

    rules = merge_rules(rectangles)
    fills = build_fill_candidates(rectangles)
    texts = build_text_candidates(text_blocks)
    bindings = build_dynamic_bindings(fields)

    pages: list[dict[str, Any]] = []
    for page_number in range(1, PAGE_COUNT + 1):
        pages.append(
            {
                "page": page_number,
                "width_pt": PAGE_WIDTH_PT,
                "height_pt": PAGE_HEIGHT_PT,
                "rules": [item for item in rules if item["page"] == page_number],
                "fill_candidates": [
                    item for item in fills if item["page"] == page_number
                ],
                "text_candidates": [
                    item for item in texts if item["page"] == page_number
                ],
                "dynamic_bindings": [
                    item for item in bindings if item["page"] == page_number
                ],
            }
        )

    return {
        "schema_version": 1,
        "artifact_kind": "review_only_owned_layout_candidate",
        "form": {
            "form_id": EXPECTED_FORM_ID,
            "form_code": "2551Q",
            "revision": "2018",
            "title": documents.metadata.get("title"),
        },
        "generator": {
            "name": GENERATOR_NAME,
            "version": GENERATOR_VERSION,
            "rounding_tolerance_pt": ROUNDING_TOLERANCE_PT,
            "classification": {
                "rule_max_thickness_pt": RULE_MAX_THICKNESS_PT,
                "rule_min_length_pt": RULE_MIN_LENGTH_PT,
                "rule_axis_tolerance_pt": RULE_AXIS_TOLERANCE_PT,
                "rule_merge_gap_pt": RULE_MERGE_GAP_PT,
                "minimum_fill_area_pt2": MIN_FILL_AREA_PT2,
                "minimum_fill_edge_pt": MIN_FILL_EDGE_PT,
                "binding_overlap_tolerance_pt": BINDING_OVERLAP_TOLERANCE_PT,
            },
        },
        "source_provenance": {
            "official_source_url": documents.metadata["official_source"],
            "official_source_sha256": documents.metadata["sha256"].lower(),
            "input_sha256": dict(sorted(documents.hashes.items())),
        },
        "coordinate_system": {
            "unit": "point",
            "origin": "top_left",
            "x_axis": "increases_rightward",
            "y_axis": "increases_downward",
            "page_width_pt": PAGE_WIDTH_PT,
            "page_height_pt": PAGE_HEIGHT_PT,
            "page_count": PAGE_COUNT,
            "text_y_semantics": (
                "baseline_y_pt is the extracted text baseline; curated CSS must apply "
                "an explicit font-metric baseline conversion"
            ),
            "field_box_semantics": (
                "x_pt and y_pt are the calibrated value-widget top-left anchor; widget "
                "dimensions do not imply an official row boundary"
            ),
            "source_bounds_tolerance_pt": SOURCE_BOUNDS_TOLERANCE_PT,
        },
        "validation": {
            "expected_geometry": "612x936pt_two_pages",
            "source_hash": "verified",
            "source_bounds": "verified",
            "dynamic_binding_overlaps": "none",
        },
        "summary": {
            "source_rectangles": len(rectangles),
            "source_text_blocks": len(text_blocks),
            "source_field_fragments": len(fields),
            "merged_rules": len(rules),
            "fill_candidates": len(fills),
            "text_candidates": len(texts),
            "dynamic_bindings": len(bindings),
            "dynamic_binding_fragments": sum(
                len(binding["fragments"]) for binding in bindings
            ),
        },
        "pages": pages,
    }


def serialize_candidate(candidate: Mapping[str, Any]) -> str:
    return json.dumps(candidate, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def _require_scratch_output(output: Path) -> Path:
    resolved = output.resolve()
    if ".scratch" not in resolved.parts:
        raise LayoutError(
            "candidate output must remain under a .scratch directory; refusing to "
            f"write {resolved}"
        )
    return resolved


def write_candidate(candidate: Mapping[str, Any], output: Path) -> Path:
    output = _require_scratch_output(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".tmp")
    temporary.write_text(serialize_candidate(candidate), encoding="utf-8")
    temporary.replace(output)
    return output


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--form-dir",
        type=Path,
        default=DEFAULT_FORM_DIR,
        help=f"calibrated source directory (default: {DEFAULT_FORM_DIR})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"review-only candidate path under .scratch (default: {DEFAULT_OUTPUT})",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the existing candidate differs instead of writing it",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        candidate = build_candidate(args.form_dir)
        serialized = serialize_candidate(candidate)
        output = _require_scratch_output(args.output)
        if args.check:
            if not output.exists():
                raise LayoutError(f"candidate does not exist for --check: {output}")
            if output.read_text(encoding="utf-8") != serialized:
                raise LayoutError(f"candidate is stale: {output}")
            print(f"owned layout candidate is current: {output}")
            return 0
        written = write_candidate(candidate, output)
        print(
            f"wrote review-only owned layout candidate: {written} "
            f"({candidate['summary']['dynamic_bindings']} bindings, "
            f"{candidate['summary']['merged_rules']} merged rules)"
        )
        return 0
    except LayoutError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
