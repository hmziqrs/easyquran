// The package's styled layer: one wrapper per `nsh/base` component in use,
// drawn with the theme's tokens so a host's theme overrides reach it.
import { type Accessor, type JSX, Text, View } from "nsh";
import { Button as BaseButton, Input } from "nsh/base";
import { type AnyFieldApi, inputProps } from "nsh-form";

const variants = {
  primary: "bg-primary active:bg-primary/90",
  outline: "border border-border bg-background hover:bg-accent active:bg-accent",
  danger: "border border-destructive bg-background active:bg-destructive/10",
};

const labels = {
  primary: "text-primary-foreground",
  outline: "text-foreground",
  danger: "text-destructive",
};

export function Button(props: {
  label: string;
  variant?: keyof typeof variants;
  disabled?: boolean;
  onPress?: () => void;
}) {
  const variant = () => props.variant ?? "primary";
  return (
    <BaseButton
      disabled={props.disabled}
      onPress={props.onPress}
      accessibilityLabel={props.label}
      class={`min-h-9 flex-row items-center justify-center gap-2 rounded-(--radius) px-4 py-2 transition-colors disabled:opacity-50 focus:outline-2 focus:outline-ring ${variants[variant()]}`}
    >
      <Text class={`text-sm font-medium ${labels[variant()]}`}>{props.label}</Text>
    </BaseButton>
  );
}

/** A labelled input: the field's value, edits, blur and its error, once left or submitted. */
export function TextField(props: { label: string; placeholder: string; field: Accessor<AnyFieldApi> }) {
  return (
    <View class="gap-1">
      <Text class="text-sm text-muted-foreground">{props.label}</Text>
      <Input
        testID={props.field().name}
        placeholder={props.placeholder}
        autoCapitalize="none"
        autoCorrect={false}
        class="min-h-9 rounded-(--radius) border border-input px-3 py-2 text-base placeholder:text-muted-foreground focus:outline-2 focus:outline-ring"
        {...inputProps(props.field)}
      />
      <If when={props.field().state.meta.errors[0] as string | undefined}>
        {(message) => <Text class="text-sm text-destructive">{message()}</Text>}
      </If>
    </View>
  );
}

export function Section(props: { title: string; children?: JSX.Element }) {
  return (
    <View class="gap-3">
      <Text role="header" class="text-lg font-semibold text-foreground">
        {props.title}
      </Text>
      {props.children}
    </View>
  );
}

export function Card(props: { children?: JSX.Element }) {
  return (
    <View class="gap-2 rounded-(--radius) border border-border bg-card p-4">{props.children}</View>
  );
}

export function Row(props: { label: string; value: string }) {
  return (
    <View class="flex-row items-center justify-between gap-3 rounded-(--radius) border border-border bg-card px-3 py-2">
      <Text class="text-foreground">{props.label}</Text>
      <Text class="text-sm text-muted-foreground">{props.value}</Text>
    </View>
  );
}
