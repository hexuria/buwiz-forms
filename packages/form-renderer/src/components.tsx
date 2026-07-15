import type { PropsWithChildren, ReactNode } from "react";

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

export function FormMasthead({
  code,
  title,
  subtitle,
  revision,
  revisionLabel,
  page,
  barcodeText
}: {
  code: string;
  title: string;
  subtitle?: string;
  revision: string;
  revisionLabel?: string;
  page: number;
  barcodeText: string;
}) {
  return (
    <header className="form-masthead">
      <div className="form-number">
        <span>BIR Form No.</span>
        <strong>{code}</strong>
        <small>{revisionLabel ?? `January ${revision} (ENCS)`}</small>
        <small>Page {page}</small>
      </div>
      <div className="form-title">
        <strong>{title}</strong>
        {subtitle && <span>{subtitle}</span>}
      </div>
      <div className="barcode" aria-label={barcodeText}>
        <span aria-hidden="true" />
        <small>{barcodeText}</small>
      </div>
    </header>
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
  className = ""
}: {
  value: string;
  cells: number;
  align?: "left" | "right";
  className?: string;
}) {
  const characters = Array.from(value);
  if (characters.length <= cells) {
    return <CombValue value={value} cells={cells} align={align} />;
  }

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
