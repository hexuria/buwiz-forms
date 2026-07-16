#!/usr/bin/env python3
"""Statically verify that the built form renderer is complete and offline-only.

This gate inspects a source/build bundle. It deliberately does not launch the
native host or exercise a network stack, so its evidence can never satisfy the
packaged-runtime promotion gate.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlsplit


HTML_REFERENCE_ATTRIBUTES = {
    "action",
    "background",
    "cite",
    "data",
    "formaction",
    "href",
    "manifest",
    "poster",
    "src",
    "xlink:href",
}
CSS_URL = re.compile(r"url\(\s*(['\"]?)(.*?)\1\s*\)", re.IGNORECASE)
CSS_IMPORT = re.compile(
    r"@import\s+(?:url\(\s*)?(['\"])(.*?)\1\s*\)?",
    re.IGNORECASE,
)
JS_IMPORT = re.compile(
    r"(?:\bimport\s*(?:\(|[^;]*?\bfrom\s*)|\bexport\s+[^;]*?\bfrom\s*)"
    r"(['\"])(.*?)\1",
)
SOURCE_MAP_REFERENCE = re.compile(
    r"[#@]\s*sourceMappingURL\s*=\s*([^\s*]+)", re.IGNORECASE
)
BUNDLE_SUFFIXES = {".js", ".mjs", ".cjs", ".css"}
ALLOWED_RUNTIME_SUFFIXES = {
    ".js",
    ".mjs",
    ".cjs",
    ".css",
    ".woff",
    ".woff2",
    ".ttf",
    ".otf",
    ".png",
}

# The native renderer is loaded from this pinned local origin/custom protocol.
# Neither origin is permitted for document references, and connect-src remains
# exactly 'none'.
REQUIRED_CSP_DIRECTIVES = {
    "default-src": {"'self'"},
    "connect-src": {"'none'"},
    "img-src": {"'self'", "data:"},
    "font-src": {"'self'"},
    "style-src": {"'self'", "'unsafe-inline'"},
    "script-src": {"'self'", "ebirforms:", "http://ebirforms.localhost"},
    "object-src": {"'none'"},
    "base-uri": {"'none'"},
    "form-action": {"'none'"},
    "frame-src": {"'none'"},
    "child-src": {"'none'"},
    "worker-src": {"'none'"},
}

# Hashes pinned by packages/form-renderer/references/manifest.json. Names are
# checked too, so copied or renamed calibration pages are both rejected.
OFFICIAL_ARTWORK_SHA256 = {
    "e62c392a3962ba4c2c31ffcb4b77a7798140473a2af99abf95173680536db599",
    "377ec4cee07cbff674686926aa0d402ec068b9a70fe3e8dbfc9802e90902f47a",
    "c78f0724e2f320f1b306408008e9085ed36397c4e1add66bf5e77c322a3485ea",
    "d6ab5afbf6b3f4cbac7c69a01df231eaf6dcf7fde587e78c02ee20e3f2508d1a",
}
OFFICIAL_SOURCE_MARKERS = {
    b"bir-cdn.bir.gov.ph",
    b"2551q%20jan%202018%20encs%20final%20rev%203_copy.pdf",
    b"packages/form-renderer/references/2551q-2018-page-",
    b"formtypes/2551qv2018/pages/page1.svg",
    b"formtypes/2551qv2018/pages/page2.svg",
    b'"official_source"',
    b'"reference_png"',
    b'"source_svg"',
}
DEV_ONLY_SAMPLE_MARKERS = {
    b"renderer preview corporation",
    b"preview@example.com",
}
RASTER_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".webp",
    ".avif",
    ".gif",
    ".bmp",
    ".ico",
}
# Only reviewed, cropped runtime artwork recorded by the generated reference
# manifest may be shipped. Keeping this allowlist derived avoids a second,
# stale authorization source whenever a form renderer is added or artwork is
# re-reviewed.
REFERENCE_MANIFEST_PATH = (
    Path(__file__).resolve().parents[1]
    / "packages/form-renderer/references/manifest.json"
)
RUNTIME_ARTWORK_SOURCE_ROOT = PurePosixPath("packages/form-renderer/src/forms")
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")


def _load_runtime_artwork_authorization(
    manifest_path: Path = REFERENCE_MANIFEST_PATH,
    workspace_root: Path | None = None,
) -> tuple[frozenset[str], tuple[str, ...]]:
    workspace = (workspace_root or manifest_path.resolve().parents[3]).resolve()
    errors: list[str] = []
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return frozenset(), (f"cannot read runtime artwork manifest: {error}",)

    forms = manifest.get("forms") if isinstance(manifest, dict) else None
    if not isinstance(forms, list) or not forms:
        return frozenset(), ("runtime artwork manifest has no forms",)

    reference_hashes: set[str] = set()
    for form in forms:
        if not isinstance(form, dict):
            continue
        pages = form.get("pages")
        if not isinstance(pages, list):
            continue
        for page in pages:
            if isinstance(page, dict) and isinstance(page.get("reference_png_sha256"), str):
                reference_hashes.add(page["reference_png_sha256"])

    authorized: set[str] = set()
    seen_assets: set[tuple[str, str, str]] = set()
    for form_index, form in enumerate(forms):
        if not isinstance(form, dict):
            errors.append(f"forms[{form_index}] is not an object")
            continue
        code = form.get("code")
        revision = form.get("revision")
        width = form.get("pages", [{}])[0].get("reference_width_px") if isinstance(form.get("pages"), list) and form.get("pages") else None
        height = form.get("pages", [{}])[0].get("reference_height_px") if isinstance(form.get("pages"), list) and form.get("pages") else None
        page_count = form.get("page_count")
        if not isinstance(code, str) or not code or not isinstance(revision, str) or not revision:
            errors.append(f"forms[{form_index}] has an invalid code or revision")
            continue
        assets = form.get("runtime_discrete_assets")
        if not isinstance(assets, list):
            errors.append(f"{code}:{revision} is missing runtime_discrete_assets")
            continue

        for asset_index, asset in enumerate(assets):
            context = f"{code}:{revision} runtime_discrete_assets[{asset_index}]"
            if not isinstance(asset, dict):
                errors.append(f"{context} is not an object")
                continue
            asset_name = asset.get("asset")
            digest = asset.get("derived_png_sha256")
            source = asset.get("embedded_in")
            crop = asset.get("crop_box_px")
            source_page = asset.get("source_page")
            if not isinstance(asset_name, str) or not asset_name:
                errors.append(f"{context} is missing asset")
            else:
                asset_key = (code, revision, asset_name)
                if asset_key in seen_assets:
                    errors.append(f"{context} duplicates asset {asset_name}")
                seen_assets.add(asset_key)
            if not isinstance(digest, str) or SHA256_PATTERN.fullmatch(digest) is None:
                errors.append(f"{context} has an invalid derived_png_sha256")
                continue
            if digest in authorized:
                errors.append(f"{context} duplicates runtime artwork hash {digest}")
            if digest in reference_hashes:
                errors.append(f"{context} attempts to authorize a full-page reference hash")

            source_path = PurePosixPath(source) if isinstance(source, str) else None
            if (
                source_path is None
                or source_path.is_absolute()
                or ".." in source_path.parts
                or source_path == RUNTIME_ARTWORK_SOURCE_ROOT
                or RUNTIME_ARTWORK_SOURCE_ROOT not in source_path.parents
                or any(part in {"reference", "references", "calibration"} for part in source_path.parts)
            ):
                errors.append(f"{context} has an invalid runtime source path")
            else:
                resolved_source = (workspace / Path(*source_path.parts)).resolve()
                try:
                    resolved_source.relative_to(workspace)
                except ValueError:
                    errors.append(f"{context} runtime source path escapes the workspace")
                else:
                    if not resolved_source.is_file():
                        errors.append(f"{context} runtime source path does not exist")
                    elif (
                        resolved_source.suffix.lower() == ".png"
                        and hashlib.sha256(resolved_source.read_bytes()).hexdigest() != digest
                    ):
                        errors.append(f"{context} runtime PNG hash does not match the manifest")

            valid_crop = (
                isinstance(crop, list)
                and len(crop) == 4
                and all(isinstance(value, int) and not isinstance(value, bool) for value in crop)
                and isinstance(width, int)
                and isinstance(height, int)
                and 0 <= crop[0] < crop[2] <= width
                and 0 <= crop[1] < crop[3] <= height
                and (crop[2] - crop[0]) * (crop[3] - crop[1]) < width * height / 4
            )
            if not valid_crop:
                errors.append(f"{context} has an invalid or full-page crop_box_px")
            if not isinstance(source_page, int) or isinstance(source_page, bool) or not isinstance(page_count, int) or not 1 <= source_page <= page_count:
                errors.append(f"{context} has an invalid source_page")

            authorized.add(digest)

    if errors:
        return frozenset(), tuple(sorted(set(errors)))
    return frozenset(authorized), ()


AUTHORIZED_RUNTIME_RASTER_SHA256, RUNTIME_ARTWORK_MANIFEST_ERRORS = (
    _load_runtime_artwork_authorization()
)
AUTHORIZED_EMBEDDED_IMAGE_SHA256: frozenset[str] = AUTHORIZED_RUNTIME_RASTER_SHA256
EMBEDDED_IMAGE_DATA = re.compile(
    rb"data\s*:\s*image(?:/|%2f)",
    re.IGNORECASE,
)
EMBEDDED_BASE64_IMAGE = re.compile(
    rb"data:image/[a-z0-9.+-]+;base64,([a-z0-9+/=]+)",
    re.IGNORECASE,
)


class ReferenceParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.references: list[tuple[str, str]] = []
        self.content_security_policies: list[str] = []
        self.inline_styles: list[tuple[str, str]] = []
        self._style_depth = 0

    def handle_starttag(
        self, tag: str, attrs: list[tuple[str, str | None]]
    ) -> None:
        tag = tag.lower()
        attributes = {name.lower(): value for name, value in attrs}
        if tag == "style":
            self._style_depth += 1
        if (
            tag == "meta"
            and (attributes.get("http-equiv") or "").lower()
            == "content-security-policy"
            and attributes.get("content")
        ):
            self.content_security_policies.append(attributes["content"] or "")
        if (
            tag == "meta"
            and (attributes.get("http-equiv") or "").lower() == "refresh"
            and attributes.get("content")
        ):
            refresh = re.search(
                r"(?:^|;)\s*url\s*=\s*(['\"]?)(.*?)\1\s*$",
                attributes["content"] or "",
                re.IGNORECASE,
            )
            if refresh:
                self.references.append(("<meta> refresh", refresh.group(2)))

        for name, value in attrs:
            attribute = name.lower()
            if not value:
                continue
            if attribute in HTML_REFERENCE_ATTRIBUTES:
                self.references.append((f"<{tag}> {attribute}", value))
            elif attribute == "srcset":
                for candidate in value.split(","):
                    reference = candidate.strip().split(maxsplit=1)[0]
                    if reference:
                        self.references.append((f"<{tag}> srcset", reference))
            elif attribute == "style":
                self.inline_styles.append((f"<{tag}> style", value))

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() == "style" and self._style_depth:
            self._style_depth -= 1

    def handle_data(self, data: str) -> None:
        if self._style_depth and data:
            self.inline_styles.append(("<style>", data))


def _parse_csp(policy: str) -> tuple[dict[str, set[str]], list[str]]:
    directives: dict[str, set[str]] = {}
    duplicates: list[str] = []
    for raw_directive in policy.split(";"):
        parts = raw_directive.strip().split()
        if not parts:
            continue
        name = parts[0].lower()
        if name in directives:
            duplicates.append(name)
        else:
            directives[name] = set(parts[1:])
    return directives, duplicates


def local_reference(reference: str) -> str | None:
    value = reference.strip()
    if not value or value.startswith("#"):
        return None
    if value.lower().startswith("data:"):
        media_type = value[5:].lstrip().lower()
        if media_type.startswith(("image/", "image%2f")):
            raise ValueError("embedded data-image URL is not allowed")
        return None
    if value.startswith("//"):
        raise ValueError("protocol-relative URL is not allowed")

    parsed = urlsplit(value)
    if parsed.scheme or parsed.netloc:
        if parsed.scheme.lower() == "file":
            raise ValueError("file URL is not allowed")
        raise ValueError("external or custom-scheme URL is not allowed")

    path = unquote(parsed.path)
    if not path:
        return None
    if "\x00" in path:
        raise ValueError("NUL byte is not allowed")
    if "\\" in path:
        raise ValueError("backslash path is not allowed")
    if path.startswith("/"):
        raise ValueError("root-absolute path is not allowed")
    normalized = Path(path)
    if ".." in normalized.parts:
        raise ValueError("parent-directory traversal is not allowed")
    return path


def resolve_reference(root: Path, source: Path, reference: str) -> Path | None:
    local = local_reference(reference)
    if local is None:
        return None
    candidate = (source.parent / local).resolve()
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise ValueError("reference escapes the renderer directory") from error
    if not candidate.is_file():
        try:
            display = candidate.relative_to(root)
        except ValueError:
            display = candidate
        raise ValueError(f"referenced file does not exist: {display}")
    return candidate


def _css_references(css: str) -> list[str]:
    references = [match.group(2) for match in CSS_URL.finditer(css)]
    references.extend(match.group(2) for match in CSS_IMPORT.finditer(css))
    references.extend(match.group(1) for match in SOURCE_MAP_REFERENCE.finditer(css))
    return sorted(set(references))


def _js_references(source: str) -> list[str]:
    references = [match.group(2) for match in JS_IMPORT.finditer(source)]
    references.extend(match.group(1) for match in SOURCE_MAP_REFERENCE.finditer(source))
    return sorted(set(references))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _raster_format(payload: bytes) -> str | None:
    """Identify common raster formats by bytes, not by attacker-controlled name."""

    if payload.startswith(b"\x89PNG\r\n\x1a\n"):
        return "PNG"
    if payload.startswith(b"\xff\xd8\xff"):
        return "JPEG"
    if len(payload) >= 12 and payload.startswith(b"RIFF") and payload[8:12] == b"WEBP":
        return "WebP"
    if (
        len(payload) >= 12
        and payload[4:8] == b"ftyp"
        and any(brand in payload[8:40] for brand in (b"avif", b"avis"))
    ):
        return "AVIF"
    if payload.startswith((b"GIF87a", b"GIF89a")):
        return "GIF"
    if payload.startswith(b"BM"):
        return "BMP"
    if payload.startswith(b"\x00\x00\x01\x00"):
        return "ICO"
    return None


def _forbidden_artwork_reason(path: Path, digest: str, payload: bytes) -> str | None:
    name = path.name.lower()
    suffix = path.suffix.lower()
    parts = {part.lower() for part in path.parts}
    if suffix == ".pdf":
        return "PDF files are calibration/source artifacts"
    raster_format = _raster_format(payload)
    if raster_format is not None and digest not in AUTHORIZED_RUNTIME_RASTER_SHA256:
        return (
            f"unauthorized {raster_format} raster payload is forbidden regardless "
            "of filename; use owned semantic HTML/CSS or a reviewed pinned asset"
        )
    if suffix in RASTER_SUFFIXES and digest not in AUTHORIZED_RUNTIME_RASTER_SHA256:
        return "unauthorized or malformed runtime raster asset"
    if digest in OFFICIAL_ARTWORK_SHA256:
        return "file matches a pinned official/reference page hash"
    if suffix == ".png" and "2551q" in name:
        return "2551Q reference PNGs are calibration-only"
    if suffix == ".svg" and (
        re.fullmatch(r"page[-_ ]?[12]\.svg", name) or "official" in name
    ):
        return "official page SVGs are calibration-only"
    if suffix in {".png", ".svg"} and parts.intersection(
        {"reference", "references", "calibration", "baselines"}
    ):
        return "reference/calibration artwork is not a runtime asset"
    if suffix == ".svg":
        header = payload[:4096].lower()
        has_page_size = re.search(
            rb"\bwidth\s*=\s*['\"]612(?:\.0+)?(?:pt)?['\"]",
            header,
        ) and re.search(
            rb"\bheight\s*=\s*['\"]936(?:\.0+)?(?:pt)?['\"]",
            header,
        )
        has_page_viewbox = re.search(
            rb"\bviewbox\s*=\s*['\"]0(?:\.0+)?[ ,]+0(?:\.0+)?[ ,]+"
            rb"612(?:\.0+)?[ ,]+936(?:\.0+)?['\"]",
            header,
        )
        if has_page_size or has_page_viewbox:
            return "SVG has the official 612x936pt page geometry"
    return None


def _scan_forbidden_runtime_sources(root: Path, files: list[Path]) -> list[str]:
    errors: list[str] = []
    for file_path in files:
        relative = file_path.relative_to(root)
        try:
            raw_payload = file_path.read_bytes()
        except OSError as error:
            errors.append(f"cannot read {relative}: {error}")
            continue
        digest = hashlib.sha256(raw_payload).hexdigest()
        if reason := _forbidden_artwork_reason(relative, digest, raw_payload):
            errors.append(f"forbidden runtime artwork {relative}: {reason}")

        payload = raw_payload.lower()
        # Validate every data-image marker independently. Looking only for any
        # base64 match would let an authorized image mask a second unsupported
        # data URI in the same bundle file.
        for marker in EMBEDDED_IMAGE_DATA.finditer(raw_payload):
            match = EMBEDDED_BASE64_IMAGE.match(raw_payload, marker.start())
            if match is None:
                errors.append(
                    f"forbidden embedded data-image runtime artwork in {relative}: "
                    "only reviewed base64 image payloads are allowed"
                )
                continue
            try:
                decoded = base64.b64decode(match.group(1), validate=True)
            except (ValueError, binascii.Error):
                errors.append(
                    f"forbidden embedded data-image runtime artwork in {relative}: "
                    "invalid base64 payload"
                )
                continue
            embedded_digest = hashlib.sha256(decoded).hexdigest()
            if embedded_digest not in AUTHORIZED_EMBEDDED_IMAGE_SHA256:
                errors.append(
                    f"forbidden embedded data-image runtime artwork in {relative}: "
                    f"unreviewed payload sha256 {embedded_digest}"
                )
        for marker in sorted(DEV_ONLY_SAMPLE_MARKERS):
            if marker in payload:
                errors.append(
                    f"development sample marker in shipped bundle {relative}: "
                    f"{marker.decode('ascii')}"
                )
        for marker in sorted(OFFICIAL_SOURCE_MARKERS):
            if marker in payload:
                errors.append(
                    f"forbidden official-source marker in {relative}: "
                    f"{marker.decode('ascii')}"
                )
    return errors


def verify_renderer(renderer_dir: Path) -> list[str]:
    root = renderer_dir.resolve()
    errors = [
        f"runtime artwork authorization manifest invalid: {error}"
        for error in RUNTIME_ARTWORK_MANIFEST_ERRORS
    ]
    if not root.is_dir():
        return [f"renderer directory does not exist: {renderer_dir}"]

    entries = sorted(root.rglob("*"))
    for symlink in (path for path in entries if path.is_symlink()):
        errors.append(f"renderer bundle must not contain symlinks: {symlink.relative_to(root)}")
    files = sorted(
        path for path in entries if not path.is_symlink() and path.is_file()
    )
    html_files = [path for path in files if path.suffix.lower() == ".html"]
    index = root / "index.html"
    if html_files != [index]:
        names = ", ".join(str(path.relative_to(root)) for path in html_files) or "none"
        errors.append(f"expected exactly one root index.html; found: {names}")
    if not index.is_file():
        return sorted(set(errors or ["renderer index.html is missing"]))
    for asset in files:
        if asset == index:
            continue
        if asset.suffix.lower() not in ALLOWED_RUNTIME_SUFFIXES:
            errors.append(
                "renderer bundle contains an unauthorized asset type: "
                f"{asset.relative_to(root)}"
            )

    parser = ReferenceParser()
    try:
        parser.feed(index.read_text(encoding="utf-8"))
    except (OSError, UnicodeError) as error:
        errors.append(f"cannot read index.html: {error}")
        return sorted(set(errors))

    if len(parser.content_security_policies) != 1:
        errors.append("index.html must define exactly one Content-Security-Policy meta tag")
    else:
        directives, duplicates = _parse_csp(parser.content_security_policies[0])
        for duplicate in duplicates:
            errors.append(f"Content-Security-Policy repeats directive {duplicate}")
        missing = sorted(set(REQUIRED_CSP_DIRECTIVES) - set(directives))
        unexpected = sorted(set(directives) - set(REQUIRED_CSP_DIRECTIVES))
        if missing:
            errors.append(f"Content-Security-Policy is missing directives: {', '.join(missing)}")
        if unexpected:
            errors.append(
                "Content-Security-Policy has unexpected directives: "
                + ", ".join(unexpected)
            )
        for directive, expected_values in REQUIRED_CSP_DIRECTIVES.items():
            if directive in directives and directives[directive] != expected_values:
                errors.append(
                    f"Content-Security-Policy requires {directive} "
                    f"{' '.join(sorted(expected_values))}"
                )

    roots: set[Path] = set()
    dependency_edges: dict[Path, set[Path]] = {}
    local_scripts = 0
    local_styles = 0

    for context, reference in parser.references:
        try:
            target = resolve_reference(root, index, reference)
        except ValueError as error:
            errors.append(f"index.html {context}={reference!r}: {error}")
            continue
        if target is None:
            continue
        roots.add(target)
        suffix = target.suffix.lower()
        local_scripts += suffix in {".js", ".mjs", ".cjs"}
        local_styles += suffix == ".css"

    for context, css in parser.inline_styles:
        for reference in _css_references(css):
            try:
                target = resolve_reference(root, index, reference)
                if target is not None:
                    roots.add(target)
            except ValueError as error:
                errors.append(f"index.html {context} reference {reference!r}: {error}")

    if local_scripts == 0:
        errors.append("index.html does not reference a local JavaScript bundle")
    if local_styles == 0:
        errors.append("index.html does not reference a local stylesheet")

    for asset in files:
        suffix = asset.suffix.lower()
        if suffix not in BUNDLE_SUFFIXES:
            continue
        try:
            source = asset.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"cannot read {asset.relative_to(root)}: {error}")
            continue
        references = _css_references(source) if suffix == ".css" else _js_references(source)
        for reference in references:
            try:
                target = resolve_reference(root, asset, reference)
                if target is not None:
                    dependency_edges.setdefault(asset.resolve(), set()).add(target)
            except ValueError as error:
                errors.append(
                    f"{asset.relative_to(root)} reference {reference!r}: {error}"
                )

    reachable = {path.resolve() for path in roots}
    pending = list(reachable)
    while pending:
        source = pending.pop()
        for target in dependency_edges.get(source, set()):
            if target not in reachable:
                reachable.add(target)
                pending.append(target)

    bundle_assets = {path.resolve() for path in files if path != index}
    for stale_asset in sorted(bundle_assets - reachable):
        errors.append(f"unreferenced bundle asset: {stale_asset.relative_to(root)}")

    errors.extend(_scan_forbidden_runtime_sources(root, files))
    return sorted(set(errors))


def build_evidence(renderer_dir: Path, errors: list[str]) -> dict:
    root = renderer_dir.resolve()
    files = []
    if root.is_dir():
        for file_path in sorted(root.rglob("*")):
            if file_path.is_symlink():
                target = file_path.readlink().as_posix().encode("utf-8")
                files.append(
                    {
                        "path": file_path.relative_to(root).as_posix(),
                        "size_bytes": len(target),
                        "sha256": hashlib.sha256(target).hexdigest(),
                        "type": "symlink",
                    }
                )
                continue
            if not file_path.is_file():
                continue
            files.append(
                {
                    "path": file_path.relative_to(root).as_posix(),
                    "size_bytes": file_path.stat().st_size,
                    "sha256": sha256_file(file_path),
                    "type": "file",
                }
            )

    bundle_digest = hashlib.sha256()
    for item in files:
        bundle_digest.update(item["path"].encode("utf-8"))
        bundle_digest.update(b"\0")
        bundle_digest.update(item["type"].encode("ascii"))
        bundle_digest.update(b"\0")
        bundle_digest.update(item["sha256"].encode("ascii"))
        bundle_digest.update(b"\n")

    normalized_errors = sorted(set(errors))
    return {
        "schema_version": 1,
        "gate": "offline_source_bundle_integrity",
        "scope": "static_source_bundle_inspection",
        "passed": not normalized_errors,
        "errors": normalized_errors,
        "network_runtime_exercised": False,
        "packaged_runtime_promotion_satisfied": False,
        "promotion_note": (
            "Static bundle inspection does not launch a packaged native host or "
            "exercise its network runtime."
        ),
        "policy": {
            "single_html_entrypoint": "index.html",
            "external_document_references_allowed": False,
            "connect_src": ["'none'"],
            "full_page_reference_artwork_allowed": False,
            "embedded_data_images_allowed": True,
            "embedded_image_sha256_allowlist": sorted(
                AUTHORIZED_EMBEDDED_IMAGE_SHA256
            ),
            "runtime_raster_assets_allowed": False,
            "runtime_raster_sha256_allowlist": sorted(
                AUTHORIZED_RUNTIME_RASTER_SHA256
            ),
            "complete_bundle_reachability_required": True,
        },
        "bundle_sha256": bundle_digest.hexdigest(),
        "files": files,
    }


def write_evidence(evidence_path: Path, evidence: dict, renderer_dir: Path) -> None:
    root = renderer_dir.resolve()
    output = evidence_path.resolve()
    try:
        output.relative_to(root)
    except ValueError:
        pass
    else:
        raise ValueError("evidence output must remain outside the renderer bundle")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "renderer_dir",
        nargs="?",
        type=Path,
        default=Path("assets/form-renderer"),
    )
    parser.add_argument(
        "--evidence-out",
        type=Path,
        help="write deterministic machine-readable static bundle evidence",
    )
    args = parser.parse_args()

    errors = verify_renderer(args.renderer_dir)
    evidence = build_evidence(args.renderer_dir, errors)
    if args.evidence_out:
        try:
            write_evidence(args.evidence_out, evidence, args.renderer_dir)
        except (OSError, ValueError) as error:
            print(f"Cannot write renderer evidence: {error}", file=sys.stderr)
            return 1

    if errors:
        print("Offline source-bundle verification failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        f"Offline source bundle verified: {args.renderer_dir} "
        f"({evidence['bundle_sha256']})"
    )
    print("Packaged/native network runtime was not exercised.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
