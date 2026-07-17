import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { assertRenderEnvelope } from "@ebirforms/form-contracts";
import {
  assertBundledPrintableFontsReady,
  FormDocument,
  geometryStabilityDecision,
  measureRenderedPages,
  rendererGeometryIsSafe,
  type RendererGeometryMeasurement
} from "@ebirforms/form-renderer";

declare global {
  interface Window {
    __EBIR_RENDER_ENVELOPE__?: unknown;
    __EBIR_RENDER_DOCUMENT_RUN_ID__?: string;
    __EBIR_RENDER_ENVELOPE_HASH__?: string;
    renderEbirForm?: (value: unknown) => void;
    measureEbirFormGeometry?: () => RendererGeometryMeasurement | null;
    prepareEbirFormForNativePrint?: (nonce: number) => void;
    ipc?: { postMessage: (message: string) => void };
  }
}

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Renderer root element is missing");
const root = createRoot(rootElement, {
  onCaughtError: (error) => reportRendererError(error),
  onUncaughtError: (error) => reportRendererError(error)
});
let measurementSequence = 0;
let renderEpoch = 0;
let hasRenderedEnvelope = false;
let pendingPrintNonce: number | undefined;
const readinessDeadlineMs = 4_000;
const nativePrintModeClass = "ebir-native-print-mode";
const rendererDocumentRunId = window.__EBIR_RENDER_DOCUMENT_RUN_ID__ ?? "";
const rendererEnvelopeHash = window.__EBIR_RENDER_ENVELOPE_HASH__ ?? "";

function postRendererHostMessage(message: Record<string, unknown>) {
  window.ipc?.postMessage(
    JSON.stringify({
      ...message,
      document_run_id: rendererDocumentRunId,
      envelope_hash: rendererEnvelopeHash
    })
  );
}

// The host-generated identity is a one-use capability for this exact WebView
// document. A reload repeats this boot message and is rejected by the host
// before a restarted render epoch can become printable.
postRendererHostMessage({ type: "renderer_boot" });

function nativePrintModeIsActive() {
  return document.documentElement.classList.contains(nativePrintModeClass);
}

function enterNativePrintMode() {
  document.documentElement.classList.add(nativePrintModeClass);
}

function leaveNativePrintMode() {
  document.documentElement.classList.remove(nativePrintModeClass);
}

function beginRendererEpoch() {
  const sequence = ++measurementSequence;
  const epoch = ++renderEpoch;
  postRendererHostMessage({
    type: "renderer_invalidated",
    render_epoch: epoch
  });
  return { sequence, epoch };
}

function render(value: unknown) {
  pendingPrintNonce = undefined;
  leaveNativePrintMode();
  hasRenderedEnvelope = false;
  try {
    assertRenderEnvelope(value);
    root.render(
      <StrictMode>
        <FormDocument envelope={value} />
      </StrictMode>
    );
    hasRenderedEnvelope = true;
    requestGeometryValidation();
  } catch (error) {
    const { epoch } = beginRendererEpoch();
    reportRendererError(error, epoch);
  }
}

function requestGeometryValidation(printNonce = pendingPrintNonce) {
  if (!hasRenderedEnvelope) return;
  const { sequence, epoch } = beginRendererEpoch();
  void waitForRenderedPages(
    sequence,
    epoch,
    performance.now() + readinessDeadlineMs,
    undefined,
    printNonce
  );
}

function nextAnimationFrame() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}

async function awaitPrintableFonts() {
  // Font metrics affect every field-cell boundary. Readiness must represent
  // the exact bundled faces, not a fallback-font layout captured one frame
  // early. FontFaceSet.ready alone also resolves after a face fails.
  await assertBundledPrintableFontsReady(document.fonts);
}

async function waitForRenderedPages(
  sequence: number,
  epoch: number,
  deadline: number,
  previousSignature?: string,
  printNonce?: number
) {
  try {
    await awaitPrintableFonts();
    if (sequence !== measurementSequence) return;
    await nextAnimationFrame();
    if (sequence !== measurementSequence) return;
    await nextAnimationFrame();
  } catch (error) {
    if (sequence !== measurementSequence) return;
    reportRendererError(error, epoch);
    return;
  }
  if (sequence !== measurementSequence) return;

  window.setTimeout(() => {
    if (sequence !== measurementSequence) return;

    const measurement = measureRenderedPages();
    if (!measurement) {
      if (performance.now() < deadline) {
        void waitForRenderedPages(
          sequence,
          epoch,
          deadline,
          previousSignature,
          printNonce
        );
        return;
      }
      reportRendererError("Renderer produced no measurable form pages", epoch);
      return;
    }
    const signature = JSON.stringify(measurement);
    const stability = geometryStabilityDecision(
      previousSignature,
      signature,
      performance.now() >= deadline
    );
    if (stability === "retry") {
      void waitForRenderedPages(sequence, epoch, deadline, signature, printNonce);
      return;
    }
    if (stability === "timed_out") {
      reportRendererError(
        "Renderer page geometry did not stabilize before the readiness deadline",
        epoch
      );
      return;
    }
    if (!rendererGeometryIsSafe(measurement)) {
      reportRendererError(
        "Renderer contains descendant overflow or clipped printable content",
        epoch
      );
      return;
    }

    const printMode = nativePrintModeIsActive();
    postRendererHostMessage({
      ...measurement,
      render_epoch: epoch,
      print_mode: printMode
    });
    postRendererHostMessage({
      type: "renderer_ready",
      render_epoch: epoch
    });
    if (
      printNonce !== undefined &&
      pendingPrintNonce === printNonce &&
      printMode
    ) {
      pendingPrintNonce = undefined;
      postRendererHostMessage({
        type: "print_ready",
        nonce: printNonce,
        render_epoch: epoch,
        print_mode: true
      });
    }
  }, 25);
}

function reportRendererError(error: unknown, epoch = renderEpoch) {
  const message = error instanceof Error ? error.message : String(error);
  const errorEpoch = epoch > 0 ? epoch : beginRendererEpoch().epoch;
  pendingPrintNonce = undefined;
  leaveNativePrintMode();
  postRendererHostMessage({
    type: "renderer_error",
    render_epoch: errorEpoch,
    message
  });
}

window.renderEbirForm = render;
window.measureEbirFormGeometry = measureRenderedPages;
window.prepareEbirFormForNativePrint = (nonce) => {
  if (!Number.isSafeInteger(nonce) || nonce <= 0) {
    reportRendererError("Native print preflight supplied an invalid nonce");
    return;
  }
  enterNativePrintMode();
  pendingPrintNonce = nonce;
  // This always invalidates previously reported readiness, waits for fonts,
  // and requires a fresh stable geometry report before native printing.
  requestGeometryValidation(nonce);
};

const invalidateForLayoutChange = () => requestGeometryValidation();
const resizeObserver =
  typeof ResizeObserver === "undefined"
    ? undefined
    : new ResizeObserver(invalidateForLayoutChange);

const refreshResizeTargets = () => {
  if (!resizeObserver) return;
  resizeObserver.disconnect();
  resizeObserver.observe(rootElement);
  rootElement
    .querySelectorAll<HTMLElement>(".form-page")
    .forEach((page) => resizeObserver.observe(page));
};

new MutationObserver(() => {
  refreshResizeTargets();
  invalidateForLayoutChange();
}).observe(rootElement, {
  attributes: true,
  characterData: true,
  childList: true,
  subtree: true
});

window.addEventListener("resize", invalidateForLayoutChange);
document.fonts?.addEventListener("loading", invalidateForLayoutChange);
document.fonts?.addEventListener("loadingdone", invalidateForLayoutChange);
document.fonts?.addEventListener("loadingerror", invalidateForLayoutChange);

const initialEnvelope = window.__EBIR_RENDER_ENVELOPE__;
if (initialEnvelope !== undefined) {
  render(initialEnvelope);
} else if (import.meta.env.DEV) {
  // Dynamic import is guarded by Vite's compile-time DEV constant, so the
  // sample and all of its taxpayer values are absent from shipped bundles.
  void import("./devPreviewEnvelope")
    .then(({ devPreviewEnvelope }) => render(devPreviewEnvelope()))
    .catch(reportRendererError);
} else {
  reportRendererError("Renderer contract envelope is missing");
}
