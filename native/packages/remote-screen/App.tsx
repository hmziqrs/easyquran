// The remote screen package: a dashboard over `nsh-query` and a small form
// over `nsh-form`, published as a signed package (scripts/delivery.sh).
import { Scroll, View, mount } from "nsh";
import { QueryClient, QueryClientProvider } from "nsh-query";
import { Dashboard } from "./src/Dashboard";
import { HostPing } from "./src/HostPing";
import { RegisterForm } from "./src/RegisterForm";
import { Section } from "./src/ui";

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <View class="flex-1 bg-background">
        <Scroll class="flex-1" contentClass="gap-6 p-4 pt-[calc(env(safe-area-inset-top)+16px)]">
          <Section title="Dashboard">
            <Dashboard />
          </Section>
          <Section title="Register a screen">
            <RegisterForm />
          </Section>
          <Section title="Host">
            <HostPing />
          </Section>
        </Scroll>
      </View>
    </QueryClientProvider>
  );
}

mount(App);
