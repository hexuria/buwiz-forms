// Responsive Buwiz Books read-only COA picker (Phase 2, ADR 0009).
//
// Reads Books' chart of accounts via the local Bot broker (/v1/ext/read, read-only)
// and stores fallback box-to-account mappings in Buwiz Forms' own database.
// Responsive layout (matches FormPage's profile-ask pattern):
// - Phone (< BREAK.phone = 600 pt): native bottom Sheet (zero extra columns on mobile).
// - Desktop/Tablet (>= 600 pt): compact Dialog over the form, reusing the exact same body.

import type { Accessor, JSX } from "nsh";
import { computed, effect, signal, Scroll, View } from "nsh";
import { bir, type BooksAccountNode, type BooksBoxMapping, type BooksCoaResponse } from "../../bir";
import { AppButton, Chip, Dialog, Line, PressCell, Sheet, Spinner, Switch, Text, TextField } from "../../ui";

/** One editable amount box on the currently open BIR form. */
export interface FormBoxOption {
  key: string;
  itemNo: string;
  label: string;
}

/** One flattened row of Books' nested COA tree for rendering. */
export interface CoaTreeRow {
  node: BooksAccountNode;
  depth: number;
  hasChildren: boolean;
}

/** Build a depth-first flattened COA tree filtered by optional search query. */
export function buildCoaTreeRows(accounts: readonly BooksAccountNode[], query: string): CoaTreeRow[] {
  const active = accounts.filter((a) => a.is_active);
  const byId = new Map<string, BooksAccountNode>();
  for (const a of active) byId.set(a.id, a);

  const children = new Map<string, BooksAccountNode[]>();
  const roots: BooksAccountNode[] = [];
  for (const a of active) {
    const pid = a.parent_account_id;
    if (pid && byId.has(pid)) {
      const list = children.get(pid) ?? [];
      list.push(a);
      children.set(pid, list);
    } else {
      roots.push(a);
    }
  }

  const byCode = (a: BooksAccountNode, b: BooksAccountNode) => a.code.localeCompare(b.code);
  roots.sort(byCode);
  for (const list of children.values()) list.sort(byCode);

  const q = query.trim().toLowerCase();
  const matches = (a: BooksAccountNode) =>
    !q ||
    a.code.toLowerCase().includes(q) ||
    a.name.toLowerCase().includes(q) ||
    a.account_type.toLowerCase().includes(q);

  const out: CoaTreeRow[] = [];
  const visit = (node: BooksAccountNode, depth: number, visited: Set<string>): boolean => {
    if (visited.has(node.id)) return false;
    visited.add(node.id);
    const kids = children.get(node.id) ?? [];
    const sub: CoaTreeRow[] = [];
    let anyKidMatch = false;
    for (const k of kids) {
      const before = out.length;
      if (visit(k, depth + 1, visited)) anyKidMatch = true;
      if (out.length > before) {
        sub.push(...out.splice(before));
      }
    }
    if (matches(node) || anyKidMatch) {
      out.push({ node, depth, hasChildren: kids.length > 0 });
      out.push(...sub);
      return true;
    }
    return false;
  };

  const visited = new Set<string>();
  for (const r of roots) visit(r, 0, visited);
  return out;
}

export interface BooksCoaPickerProps {
  open: Accessor<boolean>;
  narrow: Accessor<boolean>;
  tin: Accessor<string>;
  formCode: Accessor<string>;
  boxes: Accessor<readonly FormBoxOption[]>;
  initialBox?: Accessor<string | null>;
  onMappingsChanged?: (mappings: readonly BooksBoxMapping[]) => void;
  onClose: () => void;
  onApply: (boxKey: string, amount: string, accountLabel: string) => void;
}

export function BooksCoaPicker(props: BooksCoaPickerProps): JSX.Element {
  const selectedBox = signal<string>("");
  const query = signal<string>("");
  const includeSubtree = signal<boolean>(true);
  const loading = signal<boolean>(false);
  const error = signal<string>("");
  const coa = signal<BooksCoaResponse | null>(null);
  const mappings = signal<BooksBoxMapping[]>([]);
  const showConnect = signal<boolean>(false);
  const brokerUrl = signal<string>("http://127.0.0.1:8787");
  const extToken = signal<string>("");

  const loadData = () => {
    loading.set(true);
    error.set("");
    try {
      const saved = bir.books_box_mappings_list(props.tin(), props.formCode());
      mappings.set(saved);
    } catch {
      mappings.set([]);
    }
    try {
      const res = bir.books_coa_list();
      coa.set(res);
      showConnect.set(false);
    } catch (e) {
      coa.set(null);
      const msg = e instanceof Error ? e.message : String(e);
      error.set(msg);
      if (msg.toLowerCase().includes("not connected") || msg.toLowerCase().includes("token")) {
        showConnect.set(true);
      }
    } finally {
      loading.set(false);
    }
  };

  effect(() => {
    if (!props.open()) return;
    const requested = props.initialBox?.();
    const list = props.boxes();
    if (requested) {
      selectedBox.set(requested);
    } else if (!selectedBox() && list.length > 0) {
      selectedBox.set(list[0].key);
    }
    loadData();
  });

  const currentMapping = computed(() => {
    const k = selectedBox();
    return mappings().find((m) => m.box_key === k) ?? null;
  });

  const treeRows = computed(() => {
    const data = coa();
    if (!data) return [];
    return buildCoaTreeRows(data.accounts, query());
  });

  const connectOneClick = () => {
    error.set("");
    try {
      const url = brokerUrl().trim() || undefined;
      bir.books_broker_connect(url);
      loadData();
    } catch (e) {
      error.set(e instanceof Error ? e.message : String(e));
    }
  };

  const saveConnection = () => {
    const url = brokerUrl().trim();
    const tok = extToken().trim();
    if (!tok) {
      error.set("Enter the read-only grant token from Buwiz Bot.");
      return;
    }
    try {
      if (url) bir.set_setting("books_broker_url", url);
      bir.set_setting("books_ext_read_token", tok);
      extToken.set("");
      loadData();
    } catch (e) {
      error.set(e instanceof Error ? e.message : String(e));
    }
  };

  const pickAccount = (node: BooksAccountNode) => {
    const boxKey = selectedBox();
    const data = coa();
    if (!boxKey || !data) return;
    const sub = includeSubtree();
    const amount = sub ? node.subtree_balance : node.balance;
    try {
      bir.books_box_mapping_save({
        tin: props.tin(),
        form_type: props.formCode(),
        box_key: boxKey,
        book_id: data.book_id,
        account_id: node.id,
        account_code: node.code,
        account_name: node.name,
        account_type: node.account_type,
        include_subtree: sub,
      });
      const updated = bir.books_box_mappings_list(props.tin(), props.formCode());
      mappings.set(updated);
      props.onMappingsChanged?.(updated);
      props.onApply(boxKey, amount, `${node.code} ${node.name}`);
      props.onClose();
    } catch (e) {
      error.set(e instanceof Error ? e.message : String(e));
    }
  };

  const unlinkCurrent = () => {
    const boxKey = selectedBox();
    if (!boxKey) return;
    try {
      bir.books_box_mapping_delete(props.tin(), props.formCode(), boxKey);
      const updated = bir.books_box_mappings_list(props.tin(), props.formCode());
      mappings.set(updated);
      props.onMappingsChanged?.(updated);
    } catch (e) {
      error.set(e instanceof Error ? e.message : String(e));
    }
  };

  const body = (): JSX.Element => (
    <View class="gap-3">
      <View class="flex-row items-center justify-between gap-2">
        <View class="gap-0.5 flex-1">
          <Text variant="title">Link Books COA</Text>
          <Text variant="meta" tone="fg2">
            {coa()
              ? `${coa()!.book_name} (${coa()!.basis} basis) — read-only`
              : "Pick a Books account to fill a fallback box"}
          </Text>
        </View>
        <AppButton
          testID="books-coa-refresh"
          size="sm"
          variant="ghost"
          label="Refresh"
          onPress={loadData}
        />
      </View>

      <If when={props.boxes().length > 0}>
        <View class="gap-1.5">
          <Text variant="meta" tone="fg2">Target form box</Text>
          <Scroll horizontal class="max-h-10">
            <View class="flex-row items-center gap-1.5 pb-1">
              <Each of={props.boxes()} key={(b) => b.key}>
                {(b) => {
                  const mapped = () => mappings().some((m) => m.box_key === b().key);
                  const active = () => selectedBox() === b().key;
                  return (
                    <AppButton
                      testID={`books-box-${b().key}`}
                      size="sm"
                      variant={active() ? "primary" : "secondary"}
                      label={mapped() ? `${b().itemNo} · Linked` : `${b().itemNo} ${b().label.slice(0, 22)}`}
                      onPress={() => selectedBox.set(b().key)}
                    />
                  );
                }}
              </Each>
            </View>
          </Scroll>
        </View>
      </If>

      <If when={currentMapping()}>
        {(m) => (
          <View class="flex-row items-center justify-between gap-2 rounded-(--radius-row) border border-border bg-selected px-3 py-2">
            <View class="flex-1 gap-0.5">
              <Text variant="metaStrong">
                {`Linked: ${m().account_code} ${m().account_name}`}
              </Text>
              <Text variant="cap" tone="fg2">
                {m().include_subtree ? "Includes child sub-accounts" : "Account balance only"}
              </Text>
            </View>
            <AppButton
              testID="books-coa-unlink"
              size="sm"
              variant="ghost"
              label="Unlink"
              onPress={unlinkCurrent}
            />
          </View>
        )}
      </If>

      <If when={error()}>
        <View class="rounded-(--radius-row) border border-border bg-card px-3 py-2 gap-1">
          <Text variant="meta" tone="danger">{error()}</Text>
        </View>
      </If>

      <If when={showConnect()}>
        <View class="gap-2 rounded-(--radius-card) border border-border bg-card p-3">
          <View class="flex-row items-center justify-between gap-2">
            <View class="flex-1 gap-0.5">
              <Text variant="metaStrong">Connect local Buwiz Bot broker</Text>
              <Text variant="cap" tone="fg2">
                Loopback only (127.0.0.1). Pair in one click or enter a manual read-only grant token.
              </Text>
            </View>
            <AppButton
              testID="books-connect-auto"
              size="sm"
              variant="primary"
              label="Connect Buwiz Bot"
              onPress={connectOneClick}
            />
          </View>
          <Line />
          <TextField
            testID="books-broker-url"
            label="Broker URL"
            value={brokerUrl()}
            onChange={(v) => brokerUrl.set(v)}
          />
          <TextField
            testID="books-ext-token"
            label="Manual read-only grant token"
            value={extToken()}
            onChange={(v) => extToken.set(v)}
          />
          <View class="flex-row justify-end gap-2">
            <AppButton
              testID="books-connect-save"
              size="sm"
              variant="secondary"
              label="Save manual token"
              onPress={saveConnection}
            />
          </View>
        </View>
      </If>

      <If when={loading()}>
        <View class="flex-row items-center gap-2 py-4">
          <Spinner />
          <Text variant="meta" tone="fg2">Loading Books chart of accounts…</Text>
        </View>
      </If>

      <If when={!loading() && coa() !== null}>
        <View class="gap-2">
          <TextField
            testID="books-coa-search"
            placeholder="Filter by code, name, or type…"
            value={query()}
            onChange={(v) => query.set(v)}
          />
          <View class="flex-row items-center justify-between gap-2">
            <Switch
              testID="books-coa-subtree"
              label="Include child sub-account balances"
              checked={includeSubtree()}
              onChange={(v) => includeSubtree.set(v)}
            />
          </View>
          <Line />
          <Scroll class="max-h-72">
            <View class="gap-1">
              <Each of={treeRows()} key={(r) => r.node.id}>
                {(row) => {
                  const n = () => row().node;
                  const amt = () => (includeSubtree() ? n().subtree_balance : n().balance);
                  const isLinked = () => currentMapping()?.account_id === n().id;
                  return (
                    <PressCell
                      testID={`books-acc-${n().code}`}
                      label={`${n().code} ${n().name} ${amt()}`}
                      selected={isLinked()}
                      onPress={() => pickAccount(n())}
                      class="rounded-(--radius-row) px-2.5 py-2"
                    >
                      <View
                        class="flex-row items-center justify-between gap-2"
                        style={{ paddingLeft: row().depth * 14 }}
                      >
                        <View class="flex-1 gap-0.5">
                          <View class="flex-row items-center gap-1.5">
                            <Text variant="metaStrong" class="tabular-nums">{n().code}</Text>
                            <Text variant="body">{n().name}</Text>
                          </View>
                          <View class="flex-row items-center gap-1.5">
                            <Text variant="cap" tone="fg2">{n().account_type}</Text>
                            <If when={row().hasChildren}>
                              <Chip label="Parent" tone="neutral" />
                            </If>
                            <If when={n().accrual_only}>
                              <Chip label="Accrual-only" tone="warning" />
                            </If>
                          </View>
                        </View>
                        <Text variant="metaStrong" class="tabular-nums text-right">
                          {amt()}
                        </Text>
                      </View>
                    </PressCell>
                  );
                }}
              </Each>
              <If when={treeRows().length === 0}>
                <Text variant="meta" tone="fg2">No matching accounts.</Text>
              </If>
            </View>
          </Scroll>
        </View>
      </If>

      <View class="flex-row justify-end gap-2 pt-1">
        <AppButton testID="books-coa-close" variant="ghost" label="Close" onPress={props.onClose} />
      </View>
    </View>
  );

  return (
    <>
      <Dialog
        testID="books-coa-dialog"
        width={480}
        open={props.open() && !props.narrow()}
        onOpenChange={(o) => !o && props.onClose()}
      >
        <View class="p-5">{body()}</View>
      </Dialog>
      <Sheet
        testID="books-coa-sheet"
        open={props.open() && props.narrow()}
        onOpenChange={(o) => !o && props.onClose()}
      >
        {body()}
      </Sheet>
    </>
  );
}
