// Debug-only mount of the owner lock and Set PIN components over a recording stub interface. B3 never uses this file.
import { signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import type { BiometricKind, LockInterface, LockState, VerifyResult } from "./lockInterface";
import { LockPage } from "./LockPage";
import { SetPinPage } from "./SetPinPage";

export interface DebugLockParams {
  page?: string;
  biometric?: BiometricKind;
  wrong?: boolean;
  wait?: number;
  lost?: boolean;
}

const calls = signal<string[]>([]);
const version = signal(0);
const page = signal("lock");
let current: DebugLockParams = {};
const state = signal<LockState>({
  account_masked: "ju•••@buwiz.com",
  biometric: "face_id",
  wait_secs: 0,
  tries_left: 3,
  lost: false,
});

export function resetDebugLock(params: DebugLockParams): void {
  current = params;
  page.set(params.page ?? "lock");
  calls.set([]);
  state.set({
    account_masked: "ju•••@buwiz.com",
    biometric: params.biometric ?? "face_id",
    wait_secs: params.wait ?? 0,
    tries_left: 3,
    lost: params.lost === true,
  });
  version.set(version() + 1);
}

const stub: LockInterface = {
  lockState: () => state(),
  verify: async (_pin) => {
    calls.set([...calls(), "verify"]);
    const result: VerifyResult = current.wrong ? { kind: "wrong", tries_left: 2 } : { kind: "ok" };
    return result;
  },
  set: async (pin) => { calls.set([...calls(), `set:${pin}`]); },
  biometric: () => calls.set([...calls(), "biometric"]),
  forgot: () => calls.set([...calls(), "forgot"]),
  signOut: () => calls.set([...calls(), "signOut"]),
  clientSignIn: () => calls.set([...calls(), "clientSignIn"]),
};

export function DebugLockPages(props: DebugLockParams & { controls?: boolean }) {
  resetDebugLock(props);
  const remount = (next: Partial<LockState>) => {
    state.set({ ...state(), ...next });
    version.set(version() + 1);
  };
  return (
    <View testID="debug-lock-pages" class="flex-1 bg-background">
      <View class="flex-1">
        <If when={version() >= 0}>
          {page() === "set" ? <SetPinPage face={stub} /> : <LockPage face={stub} />}
        </If>
      </View>
      <If when={props.controls !== false}>
        <View class="absolute top-0 left-0 flex-row gap-2">
          <Button testID="fx-lock" onPress={() => { page.set("lock"); remount({ wait_secs: 0, lost: false }); }}><Text>lock</Text></Button>
          <Button testID="fx-set" onPress={() => { page.set("set"); version.set(version() + 1); }}><Text>set</Text></Button>
          <Button testID="fx-wait" onPress={() => remount({ wait_secs: 65, lost: false })}><Text>wait</Text></Button>
          <Button testID="fx-lost" onPress={() => remount({ lost: true, wait_secs: 0 })}><Text>lost</Text></Button>
          <Text testID="fx-calls">{`calls: ${calls().join(",")}`}</Text>
        </View>
      </If>
    </View>
  );
}
