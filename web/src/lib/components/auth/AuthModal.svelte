<script lang="ts">
  import { Dialog as DialogPrimitive } from "bits-ui";
  import {
    Root as Tabs,
    List as TabsList,
    Trigger as TabsTrigger,
    Content as TabsContent,
  } from "$lib/components/ui/tabs";
  import { Icon } from "$lib/components/icon";
  import { authModal } from "$lib/auth/auth-modal.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { createLoginFlow, createRegisterFlow } from "$lib/auth/flows.svelte";
  import { goto } from "$app/navigation";
  import SignInForm from "$lib/auth/components/SignInForm.svelte";
  import RegisterForm from "$lib/auth/components/RegisterForm.svelte";
  import OAuthButtons from "$lib/auth/components/OAuthButtons.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

  const loginFlow = createLoginFlow();
  const registerFlow = createRegisterFlow();

  let tab = $state<"login" | "register">("login");

  $effect(() => {
    if (authModal.open) tab = authModal.mode;
  });

  async function onAuthSuccess(): Promise<void> {
    authModal.close();
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
  }
</script>

<DialogPrimitive.Root bind:open={authModal.open}>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay
      data-slot="auth-overlay"
      class="fixed inset-0 z-[90] bg-black/55 supports-backdrop-filter:backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0"
    />
    <DialogPrimitive.Content
      data-slot="auth-dialog"
      lang={copy.locale}
      dir={copy.direction}
      class="fixed left-1/2 top-[10vh] z-[91] w-[calc(100vw-2rem)] max-w-[440px] -translate-x-1/2 overflow-hidden rounded-xl border border-line-2 bg-bg-elev shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
    >
      <DialogPrimitive.Title class="sr-only">{copy.dialogTitle}</DialogPrimitive.Title>
      <DialogPrimitive.Description class="sr-only"
        >{copy.dialogDescription}</DialogPrimitive.Description
      >
      <DialogPrimitive.Close
        class="absolute right-3.5 top-3.5 inline-flex h-8 w-8 items-center justify-center rounded-md text-fg-3 transition-colors hover:bg-bg-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        aria-label={copy.close}><Icon name="x" size={18} /></DialogPrimitive.Close
      >
      <div class="max-h-[82vh] overflow-y-auto px-6 py-7">
        <Tabs value={tab} onValueChange={(v) => (tab = v as "login" | "register")} class="flex flex-col gap-5">
          <TabsList class="flex gap-5 border-b border-line-2">
            <TabsTrigger
              value="login"
              class="relative -mb-px border-b-2 border-b-transparent px-1 pb-2.5 pt-1 text-sm text-fg-3 transition-colors hover:text-fg data-[state=active]:border-b-accent data-[state=active]:text-fg"
              >{copy.signIn}</TabsTrigger
            >
            <TabsTrigger
              value="register"
              class="relative -mb-px border-b-2 border-b-transparent px-1 pb-2.5 pt-1 text-sm text-fg-3 transition-colors hover:text-fg data-[state=active]:border-b-accent data-[state=active]:text-fg"
              >{copy.createAccount}</TabsTrigger
            >
          </TabsList>
          <TabsContent value="login">
            <SignInForm flow={loginFlow} variant="modal" onsuccess={onAuthSuccess} />
          </TabsContent>
          <TabsContent value="register">
            <RegisterForm flow={registerFlow} variant="modal" onsuccess={onAuthSuccess} />
          </TabsContent>
        </Tabs>
        <div class="mt-5">
          <OAuthButtons onPasskeySuccess={onAuthSuccess} />
        </div>
      </div>
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>
