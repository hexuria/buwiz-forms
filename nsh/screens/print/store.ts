// The previews the host has been told of, one per form code, as signals the preview window reads. One owner: only
// host.tsx writes what Rust says (`apply`, `remove`); the window only counts its page loads (`noteLoad`).
import { signal } from "nsh";
import type { PrintEvent } from "./native";

/** What a preview window shows: the sheet's file and the newest script for its page. */
export interface Preview {
  key: string;
  /** The window's title: "BIR Form 2551Q · Q1 2026" (never the taxpayer). */
  title: string;
  /** The file URL the web view loads. */
  url: string;
  /** How many documents it has shown; the script of the last one is `script`. */
  revision: number;
  /** Brings the open page to the newest document (empty until the first change after opening). */
  script: string;
  pages: number;
  paper: string;
  sheet: string;
  zoom: number;
  /** How many times the page was loaded (a live update never reloads it, so this stays 1: the scroll position is kept). */
  loads: number;
}

const all = signal<Record<string, Preview>>({});

export const previews = {
  /** The preview of a form code, or undefined once its window closed. */
  get(key: string): Preview | undefined {
    return all()[key];
  },

  /** The form codes with a preview. */
  keys(): string[] {
    return Object.keys(all());
  },

  /** Takes in an "open" or "update" event: a new preview, or the newer document of the one that is open. */
  apply(event: PrintEvent, windowTitle?: string): void {
    all.update((now) => ({
      ...now,
      [event.key]: {
        key: event.key,
        title: windowTitle ?? now[event.key]?.title ?? event.title,
        url: event.url,
        revision: event.revision,
        script: event.kind === "update" ? event.script : (now[event.key]?.script ?? ""),
        pages: event.pages,
        paper: event.paper,
        sheet: event.sheet,
        zoom: event.zoom,
        loads: now[event.key]?.loads ?? 0,
      },
    }));
  },

  /** The preview's page finished loading (its window calls this). */
  noteLoad(key: string): void {
    all.update((now) => (now[key] ? { ...now, [key]: { ...now[key], loads: now[key].loads + 1 } } : now));
  },

  /** Forgets a preview (its window closed). */
  remove(key: string): void {
    all.update((now) => {
      const { [key]: _gone, ...rest } = now;
      return rest;
    });
  },
};
