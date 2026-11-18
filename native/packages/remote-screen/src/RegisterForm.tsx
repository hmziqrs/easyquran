import { Text, View } from "nsh";
import { createForm, minLength, required } from "nsh-form";
import { createMutation, useQueryClient } from "nsh-query";
import { ScreenTaken, registerScreen } from "./api";
import { dashboardKey } from "./Dashboard";
import { Button, TextField } from "./ui";

const nameRule = ({ value }: { value: string }) =>
  required("Name the screen")({ value }) ?? minLength(3, "Use at least 3 characters")({ value });

/** Registers a screen by name; a success invalidates the dashboard, so its next load includes it. */
export function RegisterForm() {
  const queryClient = useQueryClient();
  const register = createMutation<{ id: string }, Error, string>(() => ({
    mutationFn: (name) => registerScreen(name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: dashboardKey }),
  }));
  const form = createForm(() => ({
    defaultValues: { name: "" },
    validators: {
      // TanStack Form's server-error path: what this returns lands on the field or the form.
      onSubmitAsync: async ({ value }) => {
        try {
          await register.mutateAsync(value.name);
          return undefined;
        } catch (error) {
          if (error instanceof ScreenTaken) return { fields: { name: "That name is already registered" } };
          return { form: "Couldn't reach the dashboard. Try again." };
        }
      },
    },
  }));
  const submitting = form.useStore((state) => state.isSubmitting);
  // The form-level half of `onSubmitAsync`'s answer; TanStack stores the `form` string here.
  const formError = form.useStore((state) => state.errors.find((error) => typeof error === "string"));
  return (
    <View class="gap-3">
      <form.Field name="name" validators={{ onBlur: nameRule }}>
        {(field) => <TextField label="Screen name" placeholder="e.g. Lobby board" field={field} />}
      </form.Field>
      <Button label="Register" disabled={submitting()} onPress={() => void form.handleSubmit()} />
      <If when={formError()}>
        {(message) => <Text class="text-sm text-destructive">{message()}</Text>}
      </If>
      <If when={register.data}>
        {(data) => <Text class="text-sm text-muted-foreground">{`Registered as ${data().id}`}</Text>}
      </If>
    </View>
  );
}
