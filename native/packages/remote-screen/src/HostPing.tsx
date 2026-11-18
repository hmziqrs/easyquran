import { Text, View, host, signal } from "nsh";
import { Button } from "./ui";

// One host round trip: `remote.ping` must stay in `nsh.json` `capabilities.host`,
// or the call rejects with ERR_ACCESS_DENIED before reaching the host.
export function HostPing() {
  const available = host.available();
  const result = signal<string | null>(null);

  const ping = async () => {
    const started = Date.now();
    try {
      const answer = (await host.call("remote.ping", { at: started })) as { at: number };
      result.set(`Pong in ${Date.now() - answer.at} ms`);
    } catch (error) {
      const code = (error as { code?: string }).code;
      result.set(code ? `Ping failed: ${code}` : `Ping failed: ${String(error)}`);
    }
  };

  return (
    <View class="gap-2">
      <Text class="text-sm text-muted-foreground">
        {available ? "Embedded in a host app" : "No host on this platform"}
      </Text>
      <Button label="Ping host" variant="outline" onPress={() => void ping()} />
      <If when={result()}>{(line) => <Text class="text-sm text-foreground">{line()}</Text>}</If>
    </View>
  );
}
