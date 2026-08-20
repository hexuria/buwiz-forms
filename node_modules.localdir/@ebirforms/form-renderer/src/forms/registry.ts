import type { RenderEnvelope } from "@ebirforms/form-contracts";
import type { ComponentType } from "react";
import type { FormKey } from "@ebirforms/form-specs";
import { formKey } from "@ebirforms/form-specs";
import { Form0605 } from "./Form0605";
import { Form0619E } from "./Form0619E";
import { Form0619F } from "./Form0619F";
import { Form2551Q } from "./Form2551Q";
import { Form1601C } from "./Form1601C";
import { Form1701Q } from "./Form1701Q";
import { Form1701 } from "./Form1701";
import { Form1702RT } from "./Form1702RT";
import { Form1702MX } from "./Form1702MX";
import { Form2550Q } from "./Form2550Q";

export interface FormRendererProps {
  envelope: RenderEnvelope;
}

export const FORM_COMPONENT_REGISTRY: Readonly<
  Partial<Record<FormKey, ComponentType<FormRendererProps>>>
> = {
  "0605:1999": Form0605,
  "0619E:2018": Form0619E,
  "0619F:2018": Form0619F,
  "1601C:2018": Form1601C,
  "1701Q:2018": Form1701Q,
  "1701:2018": Form1701,
  "1702RT:2018C": Form1702RT,
  "1702MX:2018C": Form1702MX,
  "2550Q:2024": Form2550Q,
  "2551Q:2018": Form2551Q
};

export function getFormComponent(
  code: string,
  revision: string
): ComponentType<FormRendererProps> {
  const component = FORM_COMPONENT_REGISTRY[formKey(code, revision)];
  if (!component) {
    throw new Error(`Unsupported HTML form ${code} revision ${revision}`);
  }
  return component;
}

export function listFormComponentKeys(): readonly FormKey[] {
  return Object.keys(FORM_COMPONENT_REGISTRY) as FormKey[];
}
