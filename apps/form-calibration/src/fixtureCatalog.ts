export interface FixtureDescriptor {
  code: string;
  id: string;
  revision: string;
}

export interface FormFixtureGroup<T extends FixtureDescriptor> {
  code: string;
  fixtures: T[];
  id: string;
  revision: string;
}

export function groupFixturesByForm<T extends FixtureDescriptor>(
  fixtures: readonly T[]
): FormFixtureGroup<T>[] {
  const groups = new Map<string, FormFixtureGroup<T>>();

  for (const fixture of fixtures) {
    const id = formIdentity(fixture.code, fixture.revision);
    const existing = groups.get(id);
    if (existing) {
      existing.fixtures.push(fixture);
      continue;
    }

    groups.set(id, {
      code: fixture.code,
      fixtures: [fixture],
      id,
      revision: fixture.revision
    });
  }

  return Array.from(groups.values()).sort((left, right) =>
    left.code.localeCompare(right.code, undefined, { numeric: true })
      || left.revision.localeCompare(right.revision, undefined, { numeric: true })
  );
}

export function preferredFixture<T extends FixtureDescriptor>(fixtures: readonly T[]): T {
  if (fixtures.length === 0) throw new Error("A form fixture group cannot be empty");
  return fixtures.find((fixture) => fixture.id.endsWith("-normal"))
    ?? fixtures.find((fixture) => fixture.id.endsWith("-minimum"))
    ?? fixtures[0];
}

export function fixtureVariantLabel(fixture: FixtureDescriptor): string {
  const codePrefix = `${fixture.code.toLocaleLowerCase()}-`;
  const normalizedId = fixture.id.toLocaleLowerCase();
  const variant = normalizedId.startsWith(codePrefix)
    ? fixture.id.slice(codePrefix.length)
    : fixture.id;
  const title = variant === "6-rows"
    ? "Canonical 6 Rows"
    : variant === "10-rows"
      ? "10 Row Overflow"
      : variant
        .split("-")
        .filter(Boolean)
        .map(titleCaseToken)
        .join(" ");

  return `${title || "Fixture"} · ${fixture.id}.json`;
}

function formIdentity(code: string, revision: string): string {
  return `${code}:${revision}`;
}

function titleCaseToken(token: string): string {
  if (/^\d+$/u.test(token)) return token;
  if (token.toLocaleLowerCase() === "item13") return "Item 13";
  if (token.toLocaleLowerCase() === "tcc") return "TCC";
  return token.charAt(0).toLocaleUpperCase() + token.slice(1).toLocaleLowerCase();
}
