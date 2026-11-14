// What the 30 s poll tells the person when filing events appeared since the last look (old pollEvents). Pure.

/** The part of a notification the toast needs. */
export interface FreshEvent {
  title: string;
  icon: string;
}

/** One new event is announced by its own title; several by a count that points at the notifications; none say nothing. */
export function eventToast(fresh: readonly FreshEvent[]): { message: string; icon: string } | null {
  if (!fresh.length) return null;
  return {
    message: fresh.length === 1 ? fresh[0].title : `${fresh.length} filing updates. Open notifications to see them.`,
    icon: fresh[0].icon,
  };
}

/** The events of `all` that `known` has not seen; `known` learns them. */
export function takeFresh<T extends { id: string }>(known: Set<string>, all: readonly T[]): T[] {
  const fresh = all.filter((n) => !known.has(n.id));
  for (const n of fresh) known.add(n.id);
  return fresh;
}
