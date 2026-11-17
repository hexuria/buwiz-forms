// Identity and status: Avatar (a hue from a key), ContextPill, StepCircle, Chip, StatusChip, Banner.
import type { JSX } from "nsh";
import { colorScheme, Text as NText, View } from "nsh";
import { Avatar as BaseAvatar, Button } from "nsh/base";
import type { IconName } from "./icons";
import { avatarColors, type ChipTone, type ReturnStatus, initials, statusChip } from "./logic";
import { cx, FOCUS_RING, TOUCH, type Tone } from "./look";
import { Icon, Line, Text } from "./text";

/** The sizes the app draws avatars at: 20 (rows, the pill), 28 (account), 40 (page header), 56 (panel). */
export type AvatarSize = 20 | 28 | 40 | 56;
const AVATAR_FONT: Record<AvatarSize, number> = { 20: 9, 28: 11, 40: 14, 56: 19 };

/**
 * A taxpayer's initials on a circle whose hue comes from `hueKey` (the TIN root) or the name. The two tints are data
 * colors (derived from the key, as the old app's avatars), lighter in light and deeper in dark; screen readers hear the name.
 */
export function Avatar(props: { name: string; size?: AvatarSize; hueKey?: string }) {
  const size = () => props.size ?? 28;
  const tone = () => avatarColors(props.hueKey || props.name, colorScheme());
  return (
    <BaseAvatar
      name={props.name}
      class="shrink-0 overflow-hidden rounded-full"
      style={{ width: size(), height: size() }}
      fallbackClass="items-center justify-center rounded-full"
      fallbackStyle={{ backgroundColor: tone().bg }}
      fallback={
        <NText class="font-semibold" style={{ fontSize: AVATAR_FONT[size()] ?? Math.round(size() * 0.36), color: tone().fg, numberOfLines: 1 }}>
          {initials(props.name)}
        </NText>
      }
    />
  );
}

/** A numbered or icon circle (welcome steps, an empty panel): 28 by default, on the hover fill. */
export function StepCircle(props: { children?: JSX.Element; label?: string; size?: number }) {
  const size = () => props.size ?? 28;
  return (
    <View class="shrink-0 items-center justify-center rounded-full bg-hover" style={{ width: size(), height: size() }}>
      <If when={props.label !== undefined} else={props.children}>
        <Text variant="metaStrong" tone="fg2" class="font-semibold">{props.label}</Text>
      </If>
    </View>
  );
}

/** Props of ContextPill. */
export interface ContextPillProps {
  /** The taxpayer's name; with `empty` the pill offers "Add a taxpayer". */
  name?: string;
  /** The TIN as printed, "000-123-453-00000". */
  tin?: string;
  /** The avatar's hue key (the TIN root). */
  hueKey?: string;
  /** The branch code, shown alone when `compact`. */
  branchCode?: string;
  /** Narrow windows: avatar and branch code only. */
  compact?: boolean;
  /** No taxpayer yet. */
  empty?: boolean;
  /** The panel it opens is open: the selected fill, the chevron points back. */
  open?: boolean;
  testID?: string;
  onPress?: () => void;
}

/** The title bar's context pill: 30 tall, full radius, avatar 20, name 13/600, TIN 12, a chevron toward the panel. */
export function ContextPill(props: ContextPillProps) {
  return (
    <Button
      testID={props.testID}
      selected={props.open}
      accessibilityLabel={props.empty ? "Add a taxpayer" : `Tax profile: ${props.name ?? ""}`}
      class={cx(
        "min-w-0 shrink flex-row items-center gap-2 rounded-full border border-border bg-card pr-2.5 enabled:hover:border-border-strong selected:border-border-strong selected:bg-selected",
        FOCUS_RING,
        TOUCH ? "min-h-11" : "h-7.5",
        props.empty ? "pl-2.5" : "pl-1",
      )}
      style={{ maxWidth: 460 }}
      onPress={props.onPress}
    >
      <If
        when={!props.empty}
        else={
          <>
            <Icon name="plus" size={14} tone="fg3" />
            <Line variant="control" class="font-semibold">Add a taxpayer</Line>
          </>
        }
      >
        <Avatar name={props.name ?? ""} size={20} hueKey={props.hueKey} />
        <If
          when={!props.compact}
          else={<Text variant="meta" tone="fg2" class="tabular-nums">{props.branchCode ?? ""}</Text>}
        >
          <Line variant="control" class="font-semibold">{props.name ?? ""}</Line>
          <Text variant="meta" tone="fg2" lines={1} class="shrink-0 tabular-nums">{props.tin ?? ""}</Text>
        </If>
      </If>
      <Icon name={props.open ? "chevron-left" : "chevron-right"} size={14} tone="fg3" />
    </Button>
  );
}

const CHIP: Record<ChipTone, { bg: string; ink: Tone }> = {
  neutral: { bg: "bg-hover", ink: "fg2" },
  accent: { bg: "bg-accent-blue-soft", ink: "accent" },
  success: { bg: "bg-success-soft", ink: "success" },
  warning: { bg: "bg-warning-soft", ink: "warning" },
  danger: { bg: "bg-danger-soft", ink: "danger" },
};

/** A chip: 20 tall, 11/500, a soft tint of its tone, an optional 12-point icon. */
export function Chip(props: { label: string; tone?: ChipTone; icon?: IconName; testID?: string }) {
  const tone = () => CHIP[props.tone ?? "neutral"];
  return (
    <View testID={props.testID} class={cx("h-5 shrink-0 flex-row items-center gap-1 rounded-full px-2", tone().bg)}>
      <If when={props.icon}>
        <Icon name={props.icon as IconName} size={12} tone={tone().ink} />
      </If>
      <Text variant="cap" tone={tone().ink} lines={1}>{props.label}</Text>
    </View>
  );
}

/** A return's status as a chip: Not started, Draft, Queued, Submitted (a send mark), Confirmed and Paid (a check), Filed (a check), In N days / Due today, N days overdue. */
export function StatusChip(props: { status: ReturnStatus | string; days?: number }) {
  const s = () => statusChip(props.status, props.days);
  return <Chip label={s().label} tone={s().tone} icon={s().check ? "check" : s().send ? "send" : undefined} />;
}

/** A band on the accent tint: radius 12, 12 by 16 padding, items in a wrapping row (the Add year band). */
export function Banner(props: { children?: JSX.Element; testID?: string }) {
  return (
    <View testID={props.testID} class="mb-4 flex-row flex-wrap items-center gap-3 rounded-(--radius-card) bg-accent-blue-soft px-4 py-3">
      {props.children}
    </View>
  );
}
