const PRINT_FONT_FAMILY = "eBIRForms Arimo";

const LATIN_PROBE = "BIR Forms 2551Q";
const LATIN_EXTENDED_PROBE = "\u0100";

export interface PrintableFontFaceLike {
  readonly family: string;
  readonly status: string;
  readonly style: string;
  readonly weight: string;
}

export interface PrintableFontFaceSetLike {
  readonly ready: PromiseLike<unknown>;
  load(
    font: string,
    text?: string
  ): PromiseLike<readonly PrintableFontFaceLike[]>;
  check(font: string, text?: string): boolean;
}

interface RequiredPrintableFontFace {
  readonly expectedStyle: "italic" | "normal";
  readonly expectedWeight: 400 | 700;
  readonly label: string;
  readonly descriptor: string;
  readonly probe: string;
}

function requiredFace(
  label: string,
  expectedStyle: "italic" | "normal",
  expectedWeight: 400 | 700,
  subset: "Latin" | "Latin Extended",
  probe: string
): RequiredPrintableFontFace {
  return {
    expectedStyle,
    expectedWeight,
    label: `${label} ${subset}`,
    descriptor: `${expectedStyle} ${expectedWeight} 16px "${PRINT_FONT_FAMILY}"`,
    probe
  };
}

/**
 * Every style/weight used by the form renderer is checked against both WOFF2
 * subsets bundled by the production build. Bold italic is explicit because
 * the official form copy contains combined emphasis and must not be browser
 * synthesized after the other faces pass readiness.
 */
const REQUIRED_PRINTABLE_FONT_FACES: readonly RequiredPrintableFontFace[] = [
  requiredFace("normal (400)", "normal", 400, "Latin", LATIN_PROBE),
  requiredFace(
    "normal (400)",
    "normal",
    400,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace("bold (700)", "normal", 700, "Latin", LATIN_PROBE),
  requiredFace(
    "bold (700)",
    "normal",
    700,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace("italic (400)", "italic", 400, "Latin", LATIN_PROBE),
  requiredFace(
    "italic (400)",
    "italic",
    400,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace(
    "bold italic (700)",
    "italic",
    700,
    "Latin",
    LATIN_PROBE
  ),
  requiredFace(
    "bold italic (700)",
    "italic",
    700,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  )
];

function unavailableFaceError(face: RequiredPrintableFontFace): Error {
  return new Error(
    `Required bundled printable font face is unavailable: ${PRINT_FONT_FAMILY} ${face.label}`
  );
}

function normalizedFamily(family: string): string {
  const normalized = family.trim();
  if (
    normalized.length >= 2 &&
    ((normalized.startsWith('"') && normalized.endsWith('"')) ||
      (normalized.startsWith("'") && normalized.endsWith("'")))
  ) {
    return normalized.slice(1, -1);
  }
  return normalized;
}

function supportsWeight(descriptor: string, requiredWeight: 400 | 700): boolean {
  const normalized = descriptor.trim().toLowerCase();
  if (normalized === "normal") return requiredWeight === 400;
  if (normalized === "bold") return requiredWeight === 700;

  const weights = normalized
    .split(/\s+/u)
    .map(Number)
    .filter(Number.isFinite);
  if (weights.length === 1) return weights[0] === requiredWeight;
  if (weights.length === 2) {
    return requiredWeight >= weights[0] && requiredWeight <= weights[1];
  }
  return false;
}

function isExactRequiredFace(
  loadedFace: PrintableFontFaceLike,
  requiredFace: RequiredPrintableFontFace
): boolean {
  return (
    loadedFace.status === "loaded" &&
    normalizedFamily(loadedFace.family) === PRINT_FONT_FAMILY &&
    loadedFace.style.trim().toLowerCase() === requiredFace.expectedStyle &&
    supportsWeight(loadedFace.weight, requiredFace.expectedWeight)
  );
}

/**
 * Wait for and prove the exact renderer-owned font family before measuring
 * printable geometry. `FontFaceSet.ready` alone also resolves when a face
 * failed, while `check()` alone can succeed when no matching face was
 * declared. Requiring a non-empty successful `load()` plus `check()` closes
 * both fallback paths.
 */
export async function assertBundledPrintableFontsReady(
  fontFaceSet: PrintableFontFaceSetLike | undefined =
    typeof document === "undefined" ? undefined : document.fonts
): Promise<void> {
  if (!fontFaceSet) {
    throw new Error("Printable font loading API is unavailable");
  }

  try {
    await fontFaceSet.ready;
  } catch {
    throw new Error("Bundled printable font set failed to settle");
  }

  for (const face of REQUIRED_PRINTABLE_FONT_FACES) {
    let loadedFaces: readonly PrintableFontFaceLike[];
    try {
      loadedFaces = await fontFaceSet.load(face.descriptor, face.probe);
    } catch {
      throw unavailableFaceError(face);
    }

    if (
      loadedFaces.length === 0 ||
      loadedFaces.some((loadedFace) => !isExactRequiredFace(loadedFace, face))
    ) {
      throw unavailableFaceError(face);
    }

    let checked = false;
    try {
      checked = fontFaceSet.check(face.descriptor, face.probe);
    } catch {
      throw unavailableFaceError(face);
    }
    if (!checked) {
      throw unavailableFaceError(face);
    }
  }
}
