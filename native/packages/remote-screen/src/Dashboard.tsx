import { Text, View } from "nsh";
import { createQuery } from "nsh-query";
import { API, getJson, type Metric } from "./api";
import { Button, Card, Row } from "./ui";

/** The dashboard's query key; the register form invalidates it. */
export const dashboardKey = ["dashboard"] as const;

/** The remote dashboard: fresh for 30 s, no automatic retries (the Retry button refetches). */
export function Dashboard() {
  const metrics = createQuery<Metric[]>(() => ({
    queryKey: dashboardKey,
    staleTime: 30_000,
    retry: false,
    queryFn: ({ signal }) => getJson<Metric[]>(`${API}/metrics`, signal),
  }));
  return (
    <Card>
      <If when={metrics.isPending}>
        <Text class="text-sm text-muted-foreground">Loading metrics…</Text>
      </If>
      <If when={metrics.error}>
        {(error) => (
          <View class="gap-2">
            <Text class="text-sm text-destructive">{`Couldn't load the dashboard: ${String(error())}`}</Text>
            <Button label="Retry" variant="outline" onPress={() => void metrics.refetch()} />
          </View>
        )}
      </If>
      <If when={metrics.isSuccess}>
        <Each of={metrics.data ?? []} key={(metric) => metric.id}>
          {(metric) => <Row label={metric().label} value={metric().value} />}
        </Each>
      </If>
      <Button
        label="Refresh"
        variant="outline"
        disabled={metrics.isFetching}
        onPress={() => void metrics.refetch()}
      />
    </Card>
  );
}
