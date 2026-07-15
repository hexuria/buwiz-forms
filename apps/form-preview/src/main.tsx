import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { assertRenderEnvelope } from "@ebirforms/form-contracts";
import {
  FormDocument,
  geometryStabilityDecision,
  measureRenderedPages,
  rendererGeometryIsSafe,
  type RendererGeometryMeasurement
} from "@ebirforms/form-renderer";

declare global {
  interface Window {
    __EBIR_RENDER_ENVELOPE__?: unknown;
    renderEbirForm?: (value: unknown) => void;
    measureEbirFormGeometry?: () => RendererGeometryMeasurement | null;
    prepareEbirFormForNativePrint?: (nonce: number) => void;
    ipc?: { postMessage: (message: string) => void };
  }
}

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Renderer root element is missing");
const root = createRoot(rootElement, {
  onCaughtError: reportRendererError,
  onUncaughtError: reportRendererError
});
let measurementSequence = 0;
let hasRenderedEnvelope = false;
let pendingPrintNonce: number | undefined;
const readinessDeadlineMs = 4_000;
const nativePrintModeClass = "ebir-native-print-mode";

function postRendererHostMessage(message: unknown) {
  window.ipc?.postMessage(JSON.stringify(message));
}

function nativePrintModeIsActive() {
  return document.documentElement.classList.contains(nativePrintModeClass);
}

function enterNativePrintMode() {
  document.documentElement.classList.add(nativePrintModeClass);
}

function leaveNativePrintMode() {
  document.documentElement.classList.remove(nativePrintModeClass);
}

function render(value: unknown) {
  try {
    pendingPrintNonce = undefined;
    leaveNativePrintMode();
    assertRenderEnvelope(value);
    root.render(
      <StrictMode>
        <FormDocument envelope={value} />
      </StrictMode>
    );
    hasRenderedEnvelope = true;
    requestGeometryValidation();
  } catch (error) {
    reportRendererError(error);
  }
}

function requestGeometryValidation(printNonce = pendingPrintNonce) {
  if (!hasRenderedEnvelope) return;
  const sequence = ++measurementSequence;
  postRendererHostMessage({ type: "renderer_invalidated" });
  void waitForRenderedPages(
    sequence,
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
  // the final loaded faces, not a fallback-font layout captured one frame early.
  await document.fonts.ready;
}

async function waitForRenderedPages(
  sequence: number,
  deadline: number,
  previousSignature?: string,
  printNonce?: number
) {
  try {
    await awaitPrintableFonts();
    await nextAnimationFrame();
    await nextAnimationFrame();
  } catch (error) {
    reportRendererError(error);
    return;
  }

  window.setTimeout(() => {
    if (sequence !== measurementSequence) return;

    const measurement = measureRenderedPages();
    if (!measurement) {
      if (performance.now() < deadline) {
        void waitForRenderedPages(
          sequence,
          deadline,
          previousSignature,
          printNonce
        );
        return;
      }
      reportRendererError("Renderer produced no measurable form pages");
      return;
    }
    const signature = JSON.stringify(measurement);
    const stability = geometryStabilityDecision(
      previousSignature,
      signature,
      performance.now() >= deadline
    );
    if (stability === "retry") {
      void waitForRenderedPages(sequence, deadline, signature, printNonce);
      return;
    }
    if (stability === "timed_out") {
      reportRendererError("Renderer page geometry did not stabilize before the readiness deadline");
      return;
    }
    if (!rendererGeometryIsSafe(measurement)) {
      reportRendererError(
        "Renderer contains descendant overflow or clipped printable content"
      );
      return;
    }

    const printMode = nativePrintModeIsActive();
    postRendererHostMessage({ ...measurement, print_mode: printMode });
    postRendererHostMessage({ type: "renderer_ready" });
    if (
      printNonce !== undefined &&
      pendingPrintNonce === printNonce &&
      printMode
    ) {
      pendingPrintNonce = undefined;
      postRendererHostMessage({
        type: "print_ready",
        nonce: printNonce,
        print_mode: true
      });
    }
  }, 25);
}

function reportRendererError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  pendingPrintNonce = undefined;
  leaveNativePrintMode();
  postRendererHostMessage({ type: "renderer_error", message });
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

window.addEventListener("afterprint", () => {
  leaveNativePrintMode();
  if (hasRenderedEnvelope) requestGeometryValidation();
});

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
document.fonts.addEventListener("loading", invalidateForLayoutChange);
document.fonts.addEventListener("loadingdone", invalidateForLayoutChange);
document.fonts.addEventListener("loadingerror", invalidateForLayoutChange);

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
