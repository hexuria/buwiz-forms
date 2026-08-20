import type { RenderEnvelope, RenderValue } from "@ebirforms/form-contracts";

export function field(envelope: RenderEnvelope, key: string): RenderValue | undefined {
  return envelope.fields[key];
}

export function text(envelope: RenderEnvelope, key: string): string {
  const value = field(envelope, key);
  return value?.type === "text" ? value.value : "";
}

export function bool(envelope: RenderEnvelope, key: string): boolean {
  const value = field(envelope, key);
  return value?.type === "boolean" ? value.value : false;
}

export function integer(envelope: RenderEnvelope, key: string): number {
  const value = field(envelope, key);
  return value?.type === "integer" ? value.value : 0;
}

export function decimal(envelope: RenderEnvelope, key: string): number {
  const value = field(envelope, key);
  return value?.type === "decimal" ? value.value : 0;
}

export function cellText(value: RenderValue | undefined): string {
  if (!value) return "";
  if (value.type === "boolean") return value.value ? "Yes" : "No";
  return String(value.value);
}

export function cellDecimal(value: RenderValue | undefined): number {
  return value?.type === "decimal" ? value.value : 0;
}
