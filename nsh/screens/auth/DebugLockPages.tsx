// Debug-only mount of the owner lock and Set PIN components over a recording stub interface. B3 never uses this file.
import { signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { maskAccount, type BiometricKind, type LockInterface, type LockPresentation, type LockState, type VerifyResult } from "./lockInterface";
import { LockPage } from "./LockPage";
import { SetPinPage } from "./SetPinPage";

export interface DebugLockParams {
  page?: string;
  biometric?: BiometricKind;
  wrong?: boolean;
  wait?: number;
  lost?: boolean;
  refuseSet?: boolean;
  account?: string;
}

const calls = signal<string[]>([]);
const version = signal(0);
const page = signal("lock");
let current: DebugLockParams = {};
type DebugLockState = LockState & LockPresentation;

const state = signal<DebugLockState>({
  room_open: true,
  store_lost: false,
  has_pin: true,
  set_at: null,
  unlocked: false,
  failures: 0,
  wait_secs: 0,
  account_masked: maskAccount("an@example.com"),
  biometric: "face_id",
});

export function resetDebugLock(params: DebugLockParams): void {
  current = params;
  page.set(params.page ?? "lock");
  calls.set([]);
  state.set({
    room_open: true,
    store_lost: params.lost === true,
    has_pin: true,
    set_at: null,
    unlocked: false,
    failures: 0,
    wait_secs: params.wait ?? 0,
    account_masked: maskAccount(params.account ?? "an@example.com"),
    biometric: params.biometric ?? "face_id",
  });
  version.set(version() + 1);
}

const stub: LockInterface = {
  lockState: () => state(),
  verify: async (_pin) => {
    calls.set([...calls(), "verify"]);
    const result: VerifyResult = current.wrong ? { kind: "wrong" } : { kind: "ok" };
    return result;
  },
  set: async (_pin) => {
    calls.set([...calls(), "set"]);
    if (current.refuseSet === true) throw new Error("Unlock with your current PIN first.");
  },
  biometric: () => calls.set([...calls(), "biometric"]),
  forgot: () => calls.set([...calls(), "forgot"]),
  signOut: () => calls.set([...calls(), "signOut"]),
  clientSignIn: () => calls.set([...calls(), "clientSignIn"]),
};

export function DebugLockPages(props: DebugLockParams & { controls?: boolean }) {
  if (process.env.NODE_ENV === "production") return null;
  resetDebugLock(props);
  const remount = (next: Partial<LockState>) => {
    state.set({ ...state(), ...next });
    version.set(version() + 1);
  };
  return (
    <View testID="debug-lock-pages" class="flex-1 bg-background">
      <View class="flex-1">
        <If when={version() % 2 === 0}>
          {page() === "set" ? <SetPinPage face={stub} /> : <LockPage face={stub} />}
        </If>
        <If when={version() % 2 === 1}>
          {page() === "set" ? <SetPinPage face={stub} /> : <LockPage face={stub} />}
        </If>
      </View>
      <If when={props.controls !== false}>
        <View class="absolute top-0 left-0 flex-row gap-2">
          <Button testID="fx-lock" onPress={() => { page.set("lock"); remount({ wait_secs: 0, store_lost: false }); }}><Text>lock</Text></Button>
          <Button testID="fx-set" onPress={() => { page.set("set"); version.set(version() + 1); }}><Text>set</Text></Button>
          <Button testID="fx-wait" onPress={() => remount({ wait_secs: 65, store_lost: false })}><Text>wait</Text></Button>
          <Button testID="fx-lost" onPress={() => remount({ store_lost: true, wait_secs: 0 })}><Text>lost</Text></Button>
          <Text testID="fx-calls">{`calls: ${calls().join(",")}`}</Text>
        </View>
      </If>
    </View>
  );
}
