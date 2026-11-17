// errorText from store.js: the words a person reads for a host error. Pure.

/** The text of a host error: a message that starts with a function name in backticks and a colon arrives; the person needs only the text. */
export function errorText(e: unknown): string {
  const message = e && (e as { message?: unknown }).message ? (e as { message: unknown }).message : e;
  return String(message)
    .replace(/^Error:\s*/, "")
    .replace(/^`[^`]+`:\s*/, "");
}
