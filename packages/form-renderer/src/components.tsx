import {
  useEffect,
  useLayoutEffect,
  useRef,
  type PropsWithChildren,
  type ReactNode
} from "react";

const ADAPTIVE_MAX_FONT_SIZE_PX = 9.6;
const ADAPTIVE_MIN_FONT_SIZE_PX = 8;
const ADAPTIVE_FONT_STEP_PX = .5;
const ADAPTIVE_FIT_TOLERANCE_PX = 1;
const useBrowserLayoutEffect = typeof window === "undefined"
  ? useEffect
  : useLayoutEffect;

export function FolioPage({
  children,
  className = "",
  pageNumber,
  paper = "folio"
}: PropsWithChildren<{
  className?: string;
  pageNumber: number;
  paper?: "folio" | "letter" | "legal";
}>) {
  return (
    <section
      className={`form-page paper-${paper} ${className}`}
      data-page-number={pageNumber}
      data-paper={paper}
    >
      {children}
    </section>
  );
}

export function GovernmentHeader() {
  return (
    <div className="government-header">
      <span>For BIR<br />Use Only</span>
      <span>BCS/<br />Item:</span>
      <strong>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</strong>
    </div>
  );
}

export function SectionTitle({ children }: PropsWithChildren) {
  return <h2 className="section-title">{children}</h2>;
}

export function FormRow({
  number,
  label,
  children,
  className = ""
}: PropsWithChildren<{ number?: string | number; label: ReactNode; className?: string }>) {
  return (
    <div className={`form-row ${className}`}>
      <div className="row-label">
        {number !== undefined && <b>{number}</b>}
        <span>{label}</span>
      </div>
      <div className="row-value">{children}</div>
    </div>
  );
}

export function CombValue({
  value,
  cells = 14,
  align = "left"
}: {
  value: string;
  cells?: number;
  align?: "left" | "right";
}) {
  const text = combCharacters(value, cells, align);
  return (
    <span className="comb-value">
      {Array.from({ length: cells }, (_, index) => (
        <span key={index}>{text[index] ?? ""}</span>
      ))}
    </span>
  );
}

export function AdaptiveCombValue({
  value,
  cells,
  align = "left",
  className = "",
  fitToField = false,
  maxFontSizePx = ADAPTIVE_MAX_FONT_SIZE_PX,
  minFontSizePx = ADAPTIVE_MIN_FONT_SIZE_PX,
  fontStepPx = ADAPTIVE_FONT_STEP_PX
}: {
  value: string;
  cells: number;
  align?: "left" | "right";
  className?: string;
  fitToField?: boolean;
  maxFontSizePx?: number;
  minFontSizePx?: number;
  fontStepPx?: number;
}) {
  const characters = Array.from(value);
  if (characters.length <= cells) {
    return <CombValue value={value} cells={cells} align={align} />;
  }

  // Existing scaffold forms retain their prior rendering until each exact
  // revision reviews a readable floor and opts into measured fitting.
  if (!fitToField) {
    const fontSize = Math.max(4, Math.min(7.2, 7.2 * cells / characters.length));
    return (
      <span
        className={`adaptive-plain-value ${className}`.trim()}
        data-cell-capacity={cells}
        data-overflow-mode="plain"
        aria-label={value}
        style={{ fontSize: `${fontSize}pt` }}
      >
        {value}
      </span>
    );
  }

  return (
    <AdaptivePlainValue
      value={value}
      cellCapacity={cells}
      align={align}
      className={className}
      maxFontSizePx={maxFontSizePx}
      minFontSizePx={minFontSizePx}
      fontStepPx={fontStepPx}
    />
  );
}

export function AdaptivePlainValue({
  value,
  cellCapacity,
  align = "left",
  className = "",
  ariaLabel = value,
  maxFontSizePx = ADAPTIVE_MAX_FONT_SIZE_PX,
  minFontSizePx = ADAPTIVE_MIN_FONT_SIZE_PX,
  fontStepPx = ADAPTIVE_FONT_STEP_PX
}: {
  value: string;
  cellCapacity?: number;
  align?: "left" | "right";
  className?: string;
  ariaLabel?: string;
  maxFontSizePx?: number;
  minFontSizePx?: number;
  fontStepPx?: number;
}) {
  validateAdaptiveFontRange(maxFontSizePx, minFontSizePx, fontStepPx);
  const valueRef = useRef<HTMLSpanElement>(null);
  useBrowserLayoutEffect(() => {
    const element = valueRef.current;
    if (!element) return;

    let active = true;
    const resizeTarget = element.parentElement;
    let lastFittedTargetWidth: number | undefined;
    let lastFittedTargetHeight: number | undefined;
    const refit = () => {
      if (!active) return;
      fitAdaptivePlainValue(element, {
        maxFontSizePx,
        minFontSizePx,
        fontStepPx
      });
      if (resizeTarget) {
        const bounds = resizeTarget.getBoundingClientRect();
        lastFittedTargetWidth = bounds.width;
        lastFittedTargetHeight = bounds.height;
      }
    };

    refit();
    void document.fonts?.ready.then(refit);
    document.fonts?.addEventListener("loadingdone", refit);

    const resizeObserver = typeof ResizeObserver === "undefined"
      ? undefined
      : new ResizeObserver(() => {
          if (!active || !resizeTarget) return;
          const bounds = resizeTarget.getBoundingClientRect();
          if (
            lastFittedTargetWidth !== undefined &&
            lastFittedTargetHeight !== undefined &&
            Math.abs(bounds.width - lastFittedTargetWidth) <=
              ADAPTIVE_FIT_TOLERANCE_PX &&
            Math.abs(bounds.height - lastFittedTargetHeight) <=
              ADAPTIVE_FIT_TOLERANCE_PX
          ) {
            return;
          }
          refit();
        });
    // Observe the field that owns the printable footprint, not the value that
    // refit() mutates. Observing the value itself lets its font-size changes
    // schedule another refit, which can feed the document readiness observer
    // indefinitely in WKWebView even though the enclosing field is stable.
    if (resizeObserver && resizeTarget) resizeObserver.observe(resizeTarget);
    if (!resizeObserver) window.addEventListener("resize", refit);

    return () => {
      active = false;
      resizeObserver?.disconnect();
      if (!resizeObserver) window.removeEventListener("resize", refit);
      document.fonts?.removeEventListener("loadingdone", refit);
    };
  }, [align, cellCapacity, className, fontStepPx, maxFontSizePx, minFontSizePx, value]);

  return (
    <span
      ref={valueRef}
      className={`adaptive-plain-value adaptive-align-${align} ${className}`.trim()}
      data-cell-capacity={cellCapacity}
      data-adaptive-fit-state="pending"
      data-adaptive-max-font-px={maxFontSizePx}
      data-adaptive-min-font-px={minFontSizePx}
      data-adaptive-step-px={fontStepPx}
      data-overflow-mode="plain"
      aria-label={ariaLabel}
      style={{ fontSize: `${maxFontSizePx}px` }}
    >
      {value}
    </span>
  );
}

/**
 * Fit overflow text against the field that will actually be printed. The
 * official comb capacity decides only when to switch to a plain box; it must
 * never be used as a proxy for that box's available width.
 */
export function fitAdaptivePlainValue(
  element: HTMLElement,
  {
    maxFontSizePx = ADAPTIVE_MAX_FONT_SIZE_PX,
    minFontSizePx = ADAPTIVE_MIN_FONT_SIZE_PX,
    fontStepPx = ADAPTIVE_FONT_STEP_PX
  }: {
    maxFontSizePx?: number;
    minFontSizePx?: number;
    fontStepPx?: number;
  } = {}
): boolean {
  validateAdaptiveFontRange(maxFontSizePx, minFontSizePx, fontStepPx);
  element.dataset.adaptiveFitState = "pending";

  if (element.clientWidth <= 0 || element.clientHeight <= 0) {
    return false;
  }

  let fontSize = maxFontSizePx;
  const inlineJustification = element.style.justifyContent;
  // Measure from the inline start so right-aligned overflow cannot escape on
  // the unscrollable start side of a flex container.
  element.style.justifyContent = "flex-start";
  element.style.fontSize = `${fontSize}px`;

  while (
    !adaptivePlainValueFits(element) &&
    fontSize > minFontSizePx
  ) {
    fontSize = Math.max(
      minFontSizePx,
      Math.round((fontSize - fontStepPx) * 1_000) / 1_000
    );
    element.style.fontSize = `${fontSize}px`;
  }

  const fits = adaptivePlainValueFits(element);
  element.style.justifyContent = inlineJustification;
  element.dataset.adaptiveFontSizePx = fontSize.toFixed(1);
  element.dataset.adaptiveFitState = fits ? "fit" : "unresolved";
  return fits;
}

function validateAdaptiveFontRange(
  maxFontSizePx: number,
  minFontSizePx: number,
  fontStepPx: number
) {
  if (
    !Number.isFinite(maxFontSizePx) ||
    !Number.isFinite(minFontSizePx) ||
    !Number.isFinite(fontStepPx) ||
    maxFontSizePx <= 0 ||
    minFontSizePx <= 0 ||
    fontStepPx <= 0 ||
    minFontSizePx > maxFontSizePx
  ) {
    throw new Error("AdaptiveCombValue requires a positive ordered font range");
  }
}

function adaptivePlainValueFits(element: HTMLElement): boolean {
  if (
    element.scrollWidth > element.clientWidth + ADAPTIVE_FIT_TOLERANCE_PX ||
    element.scrollHeight > element.clientHeight + ADAPTIVE_FIT_TOLERANCE_PX
  ) {
    return false;
  }

  // A wrapped grid/flex item can grow beyond a fixed-height field while its
  // own client and scroll dimensions remain equal. Certify the printable
  // footprint as well as the child so that such growth cannot be reported as
  // a successful fit and later be clipped by the document geometry.
  const valueBounds = element.getBoundingClientRect();
  const textRange = document.createRange();
  textRange.selectNodeContents(element);
  for (const textBounds of textRange.getClientRects()) {
    if (
      textBounds.left < valueBounds.left - ADAPTIVE_FIT_TOLERANCE_PX ||
      textBounds.top < valueBounds.top - ADAPTIVE_FIT_TOLERANCE_PX ||
      textBounds.right > valueBounds.right + ADAPTIVE_FIT_TOLERANCE_PX ||
      textBounds.bottom > valueBounds.bottom + ADAPTIVE_FIT_TOLERANCE_PX
    ) {
      return false;
    }
  }

  const owner = element.parentElement;
  if (!owner) return true;
  const ownerBounds = owner.getBoundingClientRect();
  return valueBounds.left >= ownerBounds.left - ADAPTIVE_FIT_TOLERANCE_PX &&
    valueBounds.top >= ownerBounds.top - ADAPTIVE_FIT_TOLERANCE_PX &&
    valueBounds.right <= ownerBounds.right + ADAPTIVE_FIT_TOLERANCE_PX &&
    valueBounds.bottom <= ownerBounds.bottom + ADAPTIVE_FIT_TOLERANCE_PX;
}

export function combCharacters(
  value: string,
  cells: number,
  align: "left" | "right"
): string[] {
  if (!Number.isInteger(cells) || cells <= 0) {
    throw new Error("CombValue requires a positive integer cell count");
  }
  const characters = Array.from(value);
  if (characters.length > cells) {
    throw new Error(
      `Comb value requires ${characters.length} cells but the official field allows ${cells}`
    );
  }
  const padding = Array.from({ length: cells - characters.length }, () => " ");
  const text = align === "right"
    ? [...padding, ...characters]
    : [...characters, ...padding];
  return text;
}

export function MoneyValue({ value }: { value: number }) {
  const [whole, decimal] = formatMoneyParts(value);
  return (
    <span className="money-value">
      <CombValue value={whole} cells={11} align="right" />
      <span className="decimal-separator">.</span>
      <CombValue value={decimal} cells={2} align="right" />
    </span>
  );
}

export function formatMoneyParts(value: number): [string, string] {
  if (!Number.isFinite(value)) {
    throw new Error("MoneyValue requires a finite Rust-owned decimal");
  }
  const formatted = (Object.is(value, -0) ? 0 : value).toFixed(2);
  const [whole, decimal = "00"] = formatted.split(".");
  return [whole, decimal];
}

export function CheckChoice({
  checked,
  label
}: {
  checked: boolean;
  label: string;
}) {
  return (
    <span className="check-choice">
      <span className={checked ? "check-box checked" : "check-box"} aria-hidden="true">
        {checked ? "X" : ""}
      </span>
      {label}
    </span>
  );
}

export function Declaration() {
  return (
    <section className="declaration">
      <p>
        I/We declare under the penalties of perjury that this return, and all its attachments,
        have been made in good faith, verified by me/us, and to the best of my/our knowledge and
        belief, is true and correct.
      </p>
      <div className="signature-grid">
        <div>For Individual:<br /><br /><br />Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent</div>
        <div>For Non-Individual:<br /><br /><br />Signature over Printed Name of President/Vice President/Authorized Officer</div>
      </div>
    </section>
  );
}

export function ValidationSummary({
  issues
}: {
  issues: Array<{ field_path: string; message: string; severity: string }>;
}) {
  if (issues.length === 0) return null;
  return (
    <aside className="validation-summary">
      {issues.map((issue) => (
        <p key={`${issue.field_path}-${issue.message}`} data-severity={issue.severity}>
          <b>{issue.field_path}:</b> {issue.message}
        </p>
      ))}
    </aside>
  );
}
