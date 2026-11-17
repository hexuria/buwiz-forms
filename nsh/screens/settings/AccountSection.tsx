// Settings > Account (old accountCards): Buwiz, who is signed in, Google, Security; each only where the search keeps its row.
import { BuwizCard } from "./Buwiz";
import { GoogleBlock } from "./Google";
import { IdentityBlock } from "./Identity";
import { Labeled } from "./parts";
import { SecurityCard } from "./Security";

export function AccountSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  return (
    <>
      <If when={has("buwiz")}>
        <Labeled label="Buwiz">
          <BuwizCard />
        </Labeled>
      </If>
      <If when={has("identity")}>
        <IdentityBlock />
      </If>
      <If when={has("google")}>
        <GoogleBlock />
      </If>
      <If when={has("security")}>
        <Labeled label="Security">
          <SecurityCard />
        </Labeled>
      </If>
    </>
  );
}
