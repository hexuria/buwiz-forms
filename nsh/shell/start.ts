// The dev start route (the old BIR_SHELL_START): "tp=123456788;theme=dark;view=library;form=2551Q/2026/3" opens a screen at
// launch, so one launch gives one screenshot. App.tsx passes process.env.BIR_SHELL_START; a test fixture passes a string.
// Pure parsing here; app.ts applies the steps. Keys the frame does not know are left to screens (ignored here).

/**
 * The start route a launch may use: only where the door reports a test environment (EBIR_TEST_ENV, which every dev, test and shots
 * recipe sets). The route's access steps take secrets (signin=<root>:<pin>, override=<admin pin>, recover=<key>, setuppin=<pin>), so a
 * shipped app never reads a route out of its environment. Anywhere else: no route.
 */
export function allowedStart(raw: string | null | undefined, testEnvironment: boolean): string | undefined {
  return testEnvironment && raw ? raw : undefined;
}

/** One "key=value" step of a start route. */
export interface StartStep {
  key: string;
  value: string;
}

/** The steps of a route, in order ("a=1;b=2"); a step without "=" has an empty value; blank steps are skipped. */
export function parseStart(raw: string | null | undefined): StartStep[] {
  const steps: StartStep[] = [];
  for (const part of String(raw ?? "").split(";")) {
    const at = part.indexOf("=");
    const key = (at < 0 ? part : part.slice(0, at)).trim();
    if (!key) continue;
    steps.push({ key, value: at < 0 ? "" : part.slice(at + 1).trim() });
  }
  return steps;
}

/** A return named by a start route: form=CODE/YEAR/PERIOD[/KEY] (KEY opens exactly that stored return). */
export interface StartForm {
  code: string;
  year: number;
  period: number;
  key: number | null;
}

/** Parses "2551Q/2026/3" or "2551Q/2026/3/1201"; null when the code, the year or the period is missing or not a number. */
export function parseStartForm(value: string): StartForm | null {
  const [code, year, period, key] = value.split("/");
  const y = Number(year);
  const p = Number(period);
  if (!code || !year || period === undefined || period === "" || Number.isNaN(y) || Number.isNaN(p)) return null;
  const k = key === undefined || key === "" ? null : Number(key);
  return { code, year: y, period: p, key: k === null || Number.isNaN(k) ? null : k };
}
