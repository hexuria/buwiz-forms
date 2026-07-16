import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { assertRenderEnvelope } from "@ebirforms/form-contracts";
import { FormDocument } from "@ebirforms/form-renderer";
import { getFormSpec } from "@ebirforms/form-specs";
import migrationStatus from "../../../packages/form-specs/form-migration-status.json";
import referenceManifestSource from "../../../packages/form-renderer/references/manifest.json";
import {
  fixtureVariantLabel,
  groupFixturesByForm,
  preferredFixture
} from "./fixtureCatalog";
import "./style.css";

type ViewMode = "html" | "overlay" | "difference";

interface FixtureOption {
  code: string;
  envelope: RenderEnvelope;
  id: string;
  revision: string;
  status: (typeof migrationStatus.forms)[number] | undefined;
}

interface FormOption {
  code: string;
  fixtures: FixtureOption[];
  id: string;
  label: string;
  revision: string;
}

interface ReferencePage {
  page: number;
  reference_png: string;
}

interface ReferenceForm {
  code: string;
  page_count: number;
  page_height_pt: number;
  page_width_pt: number;
  pages: ReferencePage[];
  revision: string;
}

interface ReferenceManifest {
  dpi: number;
  forms: ReferenceForm[];
}

interface PageLayout {
  height: number;
  left: number;
  page: number;
  top: number;
  width: number;
}

const fixtureModules = import.meta.glob<unknown>(
  "../../../packages/form-contracts/fixtures/*.json",
  { eager: true, import: "default" }
);
const referenceModules = import.meta.glob<string>(
  "../../../packages/form-renderer/references/*.png",
  { eager: true, import: "default", query: "?url" }
);
const referenceManifest = referenceManifestSource as ReferenceManifest;

const referenceUrls = new Map(
  Object.entries(referenceModules).map(([path, url]) => [fileName(path), url])
);

const fixtureOptions = Object.entries(fixtureModules)
  .map(([path, value]): FixtureOption => {
    assertRenderEnvelope(value);
    const id = fileName(path).replace(/\.json$/u, "");
    const status = migrationStatus.forms.find(
      (item) => item.code === value.form.code && item.revision === value.form.version
    );
    return {
      code: value.form.code,
      envelope: value,
      id,
      revision: value.form.version,
      status
    };
  })
  .sort((left, right) =>
    left.envelope.form.code.localeCompare(right.envelope.form.code, undefined, { numeric: true })
      || left.id.localeCompare(right.id)
  );

const formOptions = groupFixturesByForm(fixtureOptions).map((group): FormOption => {
  const status = group.fixtures[0]?.status;
  const readiness = isHtmlEnabled(status) ? "HTML enabled" : "scaffold only";
  return {
    ...group,
    label: `${group.code} · revision ${group.revision} · ${readiness}`
  };
});

if (fixtureOptions.length === 0) {
  throw new Error("No form renderer fixtures were found");
}

const defaultFixture = fixtureOptions.find((item) => item.id === "2551q-6-rows")
  ?? fixtureOptions[0];
const defaultForm = formOptions.find((item) => item.fixtures.some(
  (fixture) => fixture.id === defaultFixture.id
)) ?? formOptions[0];

function CalibrationApp() {
  const [selectedFixtureId, setSelectedFixtureId] = useState(defaultFixture.id);
  const [formQuery, setFormQuery] = useState(defaultForm.label);
  const [opacity, setOpacity] = useState(0.5);
  const [page, setPage] = useState(1);
  const [pageCount, setPageCount] = useState(1);
  const [pageLayouts, setPageLayouts] = useState<PageLayout[]>([]);
  const [mode, setMode] = useState<ViewMode>("html");
  const canvasRef = useRef<HTMLElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);

  const selectedFixture = fixtureOptions.find((item) => item.id === selectedFixtureId)
    ?? defaultFixture;
  const selectedForm = formOptions.find((item) => item.fixtures.some(
    (fixture) => fixture.id === selectedFixture.id
  )) ?? defaultForm;
  const envelope = selectedFixture.envelope;
  const spec = getFormSpec(envelope.form.code, envelope.form.version);
  const referenceForm = referenceManifest.forms.find(
    (item) => item.code === envelope.form.code && item.revision === envelope.form.version
  );
  const referenceLayers = (referenceForm?.pages ?? []).flatMap((referencePage) => {
    const url = referenceUrls.get(fileName(referencePage.reference_png));
    const layout = pageLayouts.find((item) => item.page === referencePage.page);
    return url && layout ? [{ layout, page: referencePage.page, url }] : [];
  });
  const comparisonAvailable = referenceLayers.length > 0;
  const currentReferenceAvailable = referenceLayers.some((item) => item.page === page);

  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;

    const pages = Array.from(stage.querySelectorAll<HTMLElement>(".form-page"));
    const measurePages = () => {
      const layouts = pages.map((item, index) => ({
        height: item.offsetHeight,
        left: item.offsetLeft,
        page: Number(item.dataset.pageNumber) || index + 1,
        top: item.offsetTop,
        width: item.offsetWidth
      }));
      const renderedPageCount = Math.max(1, layouts.length);
      setPageLayouts(layouts);
      setPageCount(renderedPageCount);
      setPage((current) => Math.min(current, renderedPageCount));
    };

    measurePages();
    const resizeObserver = new ResizeObserver(measurePages);
    pages.forEach((item) => resizeObserver.observe(item));
    return () => resizeObserver.disconnect();
  }, [selectedFixtureId]);

  useEffect(() => {
    if (!comparisonAvailable && mode !== "html") setMode("html");
  }, [comparisonAvailable, mode]);

  useEffect(() => {
    const pages = Array.from(
      stageRef.current?.querySelectorAll<HTMLElement>(".form-page") ?? []
    );
    if (pages.length === 0) return;

    const ratios = new Map<Element, number>();
    const canvas = canvasRef.current;
    const scrollRoot = canvas && canvas.scrollHeight > canvas.clientHeight ? canvas : null;
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => ratios.set(entry.target, entry.intersectionRatio));
      const mostVisible = pages
        .map((item, index) => ({
          item,
          page: Number(item.dataset.pageNumber) || index + 1,
          ratio: ratios.get(item) ?? 0
        }))
        .filter((item) => item.ratio > 0)
        .sort((left, right) => right.ratio - left.ratio)[0];
      if (mostVisible) setPage(mostVisible.page);
    }, { root: scrollRoot, threshold: [0.05, 0.2, 0.4, 0.6, 0.8] });

    pages.forEach((item) => observer.observe(item));
    return () => observer.disconnect();
  }, [selectedFixtureId]);

  function chooseFixture(option: FixtureOption) {
    setSelectedFixtureId(option.id);
    setPage(1);
    setMode("html");
  }

  function chooseForm(option: FormOption) {
    setFormQuery(option.label);
    chooseFixture(preferredFixture(option.fixtures));
  }

  function updateFormQuery(value: string) {
    setFormQuery(value);
    const exactMatch = formOptions.find(
      (item) => item.label.toLocaleLowerCase() === value.trim().toLocaleLowerCase()
    );
    if (exactMatch) chooseForm(exactMatch);
  }

  function goToPage(requestedPage: number) {
    const nextPage = Math.min(pageCount, Math.max(1, requestedPage));
    setPage(nextPage);
    const target = stageRef.current?.querySelector<HTMLElement>(
      `.form-page[data-page-number="${nextPage}"]`
    );
    const canvas = canvasRef.current;
    if (!target || !canvas) return;

    const targetTop = target.getBoundingClientRect().top;
    const canvasTop = canvas.getBoundingClientRect().top;
    const offset = targetTop - canvasTop - 32;
    if (canvas.scrollHeight > canvas.clientHeight) {
      canvas.scrollTo({ behavior: "smooth", top: canvas.scrollTop + offset });
    } else {
      window.scrollTo({ behavior: "smooth", top: window.scrollY + offset });
    }
  }

  const referenceStatus = currentReferenceAvailable
    ? `Verified ${referenceManifest.dpi} DPI reference loaded automatically for page ${page}.`
    : referenceForm
      ? `No verified reference is registered for page ${page}; other verified pages remain comparable.`
      : "No verified reference is registered for this scaffold form.";

  return (
    <main className="calibration-shell">
      <aside className="calibration-toolbar">
        <header className="toolbar-header">
          <p className="eyebrow">Developer tool</p>
          <h1>Form calibration</h1>
          <p>Inspect semantic HTML or compare it with a verified original page.</p>
        </header>

        <section className="toolbar-section" aria-labelledby="fixture-heading">
          <h2 id="fixture-heading">Form and fixture</h2>
          <label className="field-label" htmlFor="form-combobox">Form</label>
          <input
            id="form-combobox"
            className="fixture-combobox"
            type="search"
            list="form-options"
            value={formQuery}
            autoComplete="off"
            spellCheck={false}
            aria-describedby="fixture-help"
            onFocus={(event) => event.currentTarget.select()}
            onChange={(event) => updateFormQuery(event.target.value)}
            onBlur={() => setFormQuery(selectedForm.label)}
          />
          <datalist id="form-options">
            {formOptions.map((option) => (
              <option key={option.id} value={option.label} />
            ))}
          </datalist>
          <label className="field-label fixture-variant-label" htmlFor="fixture-variant-select">
            Fixture variant
          </label>
          <select
            id="fixture-variant-select"
            className="fixture-select"
            value={selectedFixture.id}
            onChange={(event) => {
              const option = selectedForm.fixtures.find(
                (fixture) => fixture.id === event.target.value
              );
              if (option) chooseFixture(option);
            }}
          >
            {selectedForm.fixtures.map((option) => (
              <option key={option.id} value={option.id}>
                {fixtureVariantLabel(option)}
              </option>
            ))}
          </select>
          <p id="fixture-help" className="field-help">
            Search by form number, then choose one of its committed fixture variants. No upload is required.
          </p>
        </section>

        <section className="toolbar-section" aria-labelledby="page-heading">
          <div className="section-heading-row">
            <h2 id="page-heading">Page</h2>
            <span>{page} of {pageCount}</span>
          </div>
          <p className="field-help page-help">Scroll normally through every page, or use these shortcuts.</p>
          <div className="page-control">
            <button
              type="button"
              aria-label="Previous page"
              disabled={page <= 1}
              onClick={() => goToPage(page - 1)}
            >
              Previous
            </button>
            <input
              aria-label="Page number"
              type="number"
              min={1}
              max={pageCount}
              value={page}
              onChange={(event) => {
                const nextPage = Number(event.target.value);
                goToPage(nextPage);
              }}
            />
            <button
              type="button"
              aria-label="Next page"
              disabled={page >= pageCount}
              onClick={() => goToPage(page + 1)}
            >
              Next
            </button>
          </div>
        </section>

        <section className="toolbar-section" aria-labelledby="compare-heading">
          <h2 id="compare-heading">Comparison</h2>
          <div className="mode-switch" role="group" aria-label="Comparison mode">
            {(["html", "overlay", "difference"] as const).map((item) => (
              <button
                key={item}
                type="button"
                aria-pressed={mode === item}
                data-active={mode === item}
                disabled={item !== "html" && !comparisonAvailable}
                onClick={() => setMode(item)}
              >
                {modeLabel(item)}
              </button>
            ))}
          </div>
          <label className="opacity-control">
            <span>Reference opacity</span>
            <output>{Math.round(opacity * 100)}%</output>
            <input
              aria-label="Reference opacity"
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={opacity}
              disabled={mode === "html" || !comparisonAvailable}
              onInput={(event) => setOpacity(Number(event.currentTarget.value))}
            />
          </label>
          <p className={`reference-status ${currentReferenceAvailable ? "is-available" : "is-unavailable"}`}>
            {referenceStatus}
          </p>
          <p className="field-help">
            The reference is calibration evidence only. It is never used as the printable form background.
          </p>
        </section>

        <section className="fixture-explainer" aria-labelledby="fixture-data-heading">
          <h2 id="fixture-data-heading">What the JSON contains</h2>
          <p>
            The fixture supplies taxpayer, period, field, schedule, payment, and validation data.
            React and CSS own the visual structure; the form spec owns paper size and pagination.
          </p>
        </section>

        <dl className="form-facts">
          <dt>Form</dt>
          <dd>{envelope.form.code} revision {envelope.form.version}</dd>
          <dt>Title</dt>
          <dd>{spec.title}</dd>
          <dt>Fixture</dt>
          <dd><code>{selectedFixture.id}.json</code></dd>
          <dt>Status</dt>
          <dd>{statusLabel(selectedFixture.status)}</dd>
          <dt>Production route</dt>
          <dd>{selectedFixture.status?.route ?? "Unregistered"}</dd>
          <dt>Release gate</dt>
          <dd>{passesReleaseGate(selectedFixture.status) ? "Passed" : "Incomplete"}</dd>
          <dt>Paper</dt>
          <dd>{spec.paper.widthPt} × {spec.paper.heightPt} pt</dd>
        </dl>
      </aside>

      <section ref={canvasRef} className="calibration-canvas" aria-label="Rendered form pages">
        <div ref={stageRef} className="calibration-stage" data-mode={mode}>
          <FormDocument envelope={envelope} />
          {mode !== "html" && referenceLayers.map((reference) => (
            <img
              key={reference.page}
              className="reference-layer"
              src={reference.url}
              alt={`Verified ${envelope.form.code} page ${reference.page} reference`}
              style={{
                height: `${reference.layout.height}px`,
                left: `${reference.layout.left}px`,
                opacity,
                top: `${reference.layout.top}px`,
                width: `${reference.layout.width}px`
              }}
            />
          ))}
        </div>
      </section>
    </main>
  );
}

function fileName(path: string): string {
  return path.split("/").at(-1) ?? path;
}

function isHtmlEnabled(status: FixtureOption["status"]): boolean {
  return Boolean(
    status
      && status.route === "html_only"
      && status.capabilities.html_component
      && status.capabilities.html_spec
  );
}

function statusLabel(status: FixtureOption["status"]): string {
  if (!status) return "Unregistered · blocked";
  const support = status.support_level === "ImplementedInApp"
    ? "implemented in app"
    : "scaffold only";
  if (isHtmlEnabled(status)) {
    return passesReleaseGate(status)
      ? "HTML enabled · release ready"
      : `HTML enabled · ${support} · release gate incomplete`;
  }
  return status.route === "experimental"
    ? `${support} · experimental HTML · release gate incomplete`
    : `${support} · renderer disabled`;
}

function passesReleaseGate(status: FixtureOption["status"]): boolean {
  return Boolean(
    status
      && status.release_ready
      && status.support_level === "ImplementedInApp"
      && isHtmlEnabled(status)
      && Object.values(status.capabilities).every(Boolean)
  );
}

function modeLabel(mode: ViewMode): string {
  if (mode === "html") return "HTML";
  if (mode === "overlay") return "Overlay";
  return "Difference";
}

const root = document.getElementById("root");
if (!root) throw new Error("Calibration root element is missing");
createRoot(root).render(<CalibrationApp />);
