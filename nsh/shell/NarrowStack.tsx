// A phone's pages in the platform's own navigation stack (iOS: a UINavigationController; Android: its back stack), so the
// system's back gesture works: on iOS the page follows a finger dragged from the left edge, on Android the back gesture shows
// its preview. The app's history stays the one truth: the stack's pages are app.stack() (the stops since the last tab), each
// named by its route's seq. A move of the app's (go, Back in the header, a tab) changes the history and the stack follows; a
// page the person swiped away reports itself (onDismiss) and the history follows (app.pagePopped).
// The system header is hidden: the app's own header (NarrowBar) and tab bar stay around the stack.
import type { JSX } from "nsh";
import { effect, untracked } from "nsh";
import { createNavStack, NavPage, NavStack } from "nsh/base";
import { app } from "./app";
import { TABS, type Route } from "./routes";

const TAB_VIEWS: string[] = TABS.map((t) => t.view);

const seqs = (): number[] => app.stack().map((page) => page.seq);
const same = (a: readonly number[], b: readonly number[]): boolean => a.length === b.length && a.every((v, i) => v === b[i]);

export function NarrowStack(props: { page: (route: Route) => JSX.Element }) {
  const nav = createNavStack<number>(untracked(seqs));
  effect(() => {
    const next = seqs();
    untracked(() => {
      if (!same(next, nav.routes())) nav.setRoutes(next);
    });
  });
  return (
    <NavStack state={nav} testID="narrow-stack" class="min-h-0 flex-1">
      {(seq: number) => {
        const route = untracked(() => app.stackRoute(seq));
        if (!route) return <NavPage headerVisible={false} />;
        return (
          <NavPage
            headerVisible={false}
            // A tab takes the place of whatever was there, as tabs do; a pushed page slides in.
            animation={TAB_VIEWS.includes(route.view) ? "none" : "default"}
            onDismiss={() => app.pagePopped(seq)}
            class="min-h-0 flex-1 bg-card"
          >
            {props.page(route)}
          </NavPage>
        );
      }}
    </NavStack>
  );
}
