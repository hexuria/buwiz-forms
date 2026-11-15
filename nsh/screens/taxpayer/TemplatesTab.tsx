// The Templates tab (old templatesTab): the forms the branch files in the year, each with its template (when it was
// updated, how many answers and stops it fixes) and Create, Update or Delete. A template starts from the newest return.
import { View } from "nsh";
import { shortDate } from "../../lib/dates";
import { app, go, store } from "../../shell";
import { AppButton, Code, EmptyRow, ListGroup, ListRow, NameCell, Text } from "../../ui";
import { deleteTemplate, makeTemplate } from "./actions";
import { templateCounts } from "./logic";
import type { PageState } from "./page";

export function TemplatesTab(props: { s: PageState }) {
  const b = () => app.branch();
  const rows = () => {
    const codes = new Set(app.yearForms());
    return store.screenIndex().filter((f) => codes.has(f.c));
  };
  const tplOf = (code: string) => app.templates().find((x) => x.form_code === code);
  const compact = () => app.layout.mainWidth() < 600;
  return (
    <View testID="tp-templates-tab" class="w-full">
      <If
        when={app.years().length > 0}
        else={
          <ListGroup testID="tp-templates">
            <EmptyRow
              testID="tp-templates-noyear"
              text={`${b()?.name ?? ""} has no tax year yet. Add one to choose the forms it files.`}
              action={<AppButton testID="tp-templates-add-year" label="Add year" size="sm" icon="plus" onPress={() => go("forms", { addYear: true })} />}
            />
          </ListGroup>
        }
      >
        <Text tone="fg2" class="mb-4">
          {`A new return starts with the answers of your last one. A template fixes the answers to start from instead. ${b()?.name ?? ""} can have one per form, kept across tax years. These are the forms it files in ${app.year()}. A template never holds amounts or dates.`}
        </Text>
        <ListGroup testID="tp-templates">
          <If
            when={rows().length > 0}
            else={
              <EmptyRow
                testID="tp-templates-empty"
                text={`No forms for ${app.year()} yet.`}
                action={<AppButton testID="tp-templates-choose" label="Choose forms" size="sm" onPress={() => go("forms", { editForms: true })} />}
              />
            }
          >
            <Each of={rows()} key={(f) => f.c}>
              {(f) => (
                <ListRow testID={`tp-template-${f().c}`}>
                  <View class="shrink-0" style={{ width: 68 }}>
                    <Code tone={tplOf(f().c) ? "fg" : "fg3"}>{f().c}</Code>
                  </View>
                  <NameCell title={f().t} sub={tplOf(f().c) ? `Updated ${shortDate(tplOf(f().c)?.updated_at)}` : "No template yet"} dim={!tplOf(f().c)} />
                  <If when={!compact()}>
                    <View class="shrink-0" style={{ width: 176 }}>
                      <Text variant="meta" tone="fg2" class="tabular-nums">{tplOf(f().c) ? templateCounts(tplOf(f().c) as NonNullable<ReturnType<typeof tplOf>>) : ""}</Text>
                    </View>
                  </If>
                  <View class="shrink-0 flex-row justify-end gap-1">
                    <If
                      when={tplOf(f().c)}
                      else={<AppButton testID={`tp-template-${f().c}-create`} label="Create" size="sm" icon="plus" onPress={() => makeTemplate(f().c)} />}
                    >
                      <AppButton testID={`tp-template-${f().c}-update`} label="Update" variant="ghost" size="sm" onPress={() => makeTemplate(f().c)} />
                      <AppButton testID={`tp-template-${f().c}-delete`} label="Delete" variant="ghost" size="sm" onPress={() => deleteTemplate(f().c)} />
                    </If>
                  </View>
                </ListRow>
              )}
            </Each>
          </If>
        </ListGroup>
      </If>
    </View>
  );
}
