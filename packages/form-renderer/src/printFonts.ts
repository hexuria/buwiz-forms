const ARIMO_FONT_FAMILY = "eBIRForms Arimo";
const CONDENSED_FONT_FAMILY = "eBIRForms Roboto Condensed";

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
  readonly family: string;
  readonly expectedStyle: "italic" | "normal";
  readonly expectedWeight: 400 | 700;
  readonly label: string;
  readonly descriptor: string;
  readonly probe: string;
}

function requiredFace(
  family: string,
  label: string,
  expectedStyle: "italic" | "normal",
  expectedWeight: 400 | 700,
  subset: "Latin" | "Latin Extended",
  probe: string
): RequiredPrintableFontFace {
  return {
    family,
    expectedStyle,
    expectedWeight,
    label: `${label} ${subset}`,
    descriptor: `${expectedStyle} ${expectedWeight} 16px "${family}"`,
    probe
  };
}

/**
 * Every style/weight used by the form renderer is checked against its reviewed
 * WOFF2 subset. Arimo includes Latin and Latin Extended; the form-scoped
 * condensed face only needs Latin. Bold italic is explicit because official
 * form copy contains combined emphasis and must not be browser synthesized.
 */
const REQUIRED_PRINTABLE_FONT_FACES: readonly RequiredPrintableFontFace[] = [
  requiredFace(ARIMO_FONT_FAMILY, "normal (400)", "normal", 400, "Latin", LATIN_PROBE),
  requiredFace(
    ARIMO_FONT_FAMILY,
    "normal (400)",
    "normal",
    400,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace(ARIMO_FONT_FAMILY, "bold (700)", "normal", 700, "Latin", LATIN_PROBE),
  requiredFace(
    ARIMO_FONT_FAMILY,
    "bold (700)",
    "normal",
    700,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace(ARIMO_FONT_FAMILY, "italic (400)", "italic", 400, "Latin", LATIN_PROBE),
  requiredFace(
    ARIMO_FONT_FAMILY,
    "italic (400)",
    "italic",
    400,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace(
    ARIMO_FONT_FAMILY,
    "bold italic (700)",
    "italic",
    700,
    "Latin",
    LATIN_PROBE
  ),
  requiredFace(
    ARIMO_FONT_FAMILY,
    "bold italic (700)",
    "italic",
    700,
    "Latin Extended",
    LATIN_EXTENDED_PROBE
  ),
  requiredFace(
    CONDENSED_FONT_FAMILY,
    "normal (400)",
    "normal",
    400,
    "Latin",
    LATIN_PROBE
  ),
  requiredFace(
    CONDENSED_FONT_FAMILY,
    "italic (400)",
    "italic",
    400,
    "Latin",
    LATIN_PROBE
  )
];

function unavailableFaceError(face: RequiredPrintableFontFace): Error {
  return new Error(
    `Required bundled printable font face is unavailable: ${face.family} ${face.label}`
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
    normalizedFamily(loadedFace.family) === requiredFace.family &&
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
    typeof document === "undefined" ? undefined : document.fonts,
  timeoutMs = 4_000
): Promise<void> {
  if (!fontFaceSet) {
    throw new Error("Printable font loading API is unavailable");
  }
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new Error("Printable font readiness requires a positive timeout");
  }
  const deadline = Date.now() + timeoutMs;

  try {
    await settleBeforeDeadline(
      fontFaceSet.ready,
      deadline,
      "Bundled printable font set did not settle before the readiness deadline"
    );
  } catch {
    throw new Error("Bundled printable font set failed to settle");
  }

  for (const face of REQUIRED_PRINTABLE_FONT_FACES) {
    let loadedFaces: readonly PrintableFontFaceLike[];
    try {
      loadedFaces = await settleBeforeDeadline(
        fontFaceSet.load(face.descriptor, face.probe),
        deadline,
        `Required bundled printable font face did not settle: ${face.label}`
      );
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

async function settleBeforeDeadline<T>(
  value: PromiseLike<T>,
  deadline: number,
  timeoutMessage: string
): Promise<T> {
  const remainingMs = deadline - Date.now();
  if (remainingMs <= 0) throw new Error(timeoutMessage);

  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      Promise.resolve(value),
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error(timeoutMessage)), remainingMs);
      })
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
