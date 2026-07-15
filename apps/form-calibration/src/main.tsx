import { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { assertRenderEnvelope } from "@ebirforms/form-contracts";
import { FormDocument } from "@ebirforms/form-renderer";
import fixture from "../../../packages/form-contracts/fixtures/2551q-10-rows.json";
import "./style.css";

type ViewMode = "overlay" | "difference";

function CalibrationApp() {
  const [envelope, setEnvelope] = useState<RenderEnvelope>(() => {
    assertRenderEnvelope(fixture);
    return fixture;
  });
  const [referenceUrl, setReferenceUrl] = useState<string>();
  const [opacity, setOpacity] = useState(0.5);
  const [page, setPage] = useState(1);
  const [mode, setMode] = useState<ViewMode>("overlay");

  useEffect(
    () => () => {
      if (referenceUrl) URL.revokeObjectURL(referenceUrl);
    },
    [referenceUrl]
  );

  const pageSelector = useMemo(
    () => `.calibration-stage .form-page:not([data-page-number="${page}"])`,
    [page]
  );

  async function loadEnvelope(file: File) {
    const value = JSON.parse(await file.text()) as unknown;
    assertRenderEnvelope(value);
    setEnvelope(value);
    setPage(1);
  }

  function loadReference(file: File) {
    setReferenceUrl((current) => {
      if (current) URL.revokeObjectURL(current);
      return URL.createObjectURL(file);
    });
  }

  return (
    <main className="calibration-shell">
      <aside className="calibration-toolbar">
        <h1>Form calibration</h1>
        <p>Compare the semantic HTML page against a 144 DPI official reference raster.</p>
        <label>
          Contract fixture
          <input
            type="file"
            accept="application/json,.json"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) void loadEnvelope(file);
            }}
          />
        </label>
        <label>
          Reference page image
          <input
            type="file"
            accept="image/png,image/jpeg"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) loadReference(file);
            }}
          />
        </label>
        <label>
          Page
          <input
            type="number"
            min={1}
            value={page}
            onChange={(event) => setPage(Math.max(1, Number(event.target.value)))}
          />
        </label>
        <label>
          Reference opacity: {Math.round(opacity * 100)}%
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={opacity}
            onChange={(event) => setOpacity(Number(event.target.value))}
          />
        </label>
        <div className="mode-switch" role="group" aria-label="Comparison mode">
          <button type="button" data-active={mode === "overlay"} onClick={() => setMode("overlay")}>
            Overlay
          </button>
          <button
            type="button"
            data-active={mode === "difference"}
            onClick={() => setMode("difference")}
          >
            Difference
          </button>
        </div>
        <dl>
          <dt>Form</dt>
          <dd>{envelope.form.code} revision {envelope.form.version}</dd>
          <dt>Paper</dt>
          <dd>612 x 936 pt</dd>
        </dl>
      </aside>
      <section className="calibration-canvas">
        <style>{`${pageSelector} { display: none !important; }`}</style>
        <div className="calibration-stage" data-mode={mode}>
          <FormDocument envelope={envelope} />
          {referenceUrl && (
            <img
              className="reference-layer"
              src={referenceUrl}
              alt="Official form reference"
              style={{ opacity }}
            />
          )}
        </div>
      </section>
    </main>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("Calibration root element is missing");
createRoot(root).render(<CalibrationApp />);
