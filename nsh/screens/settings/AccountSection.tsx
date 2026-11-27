// Settings > Account (old accountCards): the Buwiz account, the profile on this computer (who unlocked eBIRForms here, not a
// second account), Google, Security; each only where the search keeps its row.
import { deviceCopy } from "../../ui";
import { BuwizCard } from "./Buwiz";
import { SettingsMigrationRow } from "../auth/migrationNote";
import { GoogleBlock } from "./Google";
import { IdentityBlock } from "./Identity";
import { Labeled } from "./parts";
import { SecurityCard } from "./Security";

export function AccountSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  return (
    <>
      <If when={has("buwiz")}>
        <Labeled label="Buwiz account" testID="settings-buwiz-heading">
          <BuwizCard />
          {/* The permanent failure's note lives here after Home showed it once (§A.3.4). */}
          <SettingsMigrationRow />
        </Labeled>
      </If>
      <If when={has("identity")}>
        <Labeled label={deviceCopy("Profile on this computer")} testID="settings-profile-heading">
          <IdentityBlock />
        </Labeled>
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
