// Behavior tests for the remote screen package: stubbed API routes and a
// manual clock, so nothing here reaches a network or waits on real time.
import { expect, open, test } from "nsh/test";

const API = "https://dashboard.example.com";

type Metric = { id: string; label: string; value: string };

/** A dashboard a test can grow: `POST /screens` registers a name, `GET /metrics` reports it. */
function dashboardApi() {
  const screens = ["Lobby board"];
  return {
    screens,
    routes: {
      [`GET ${API}/metrics`]: () => ({
        body: [
          { id: "m-sessions", label: "Sessions today", value: "1,284" },
          ...screens.map((name, index) => ({ id: `s-${index}`, label: name, value: "live" })),
        ] satisfies Metric[],
      }),
      [`POST ${API}/screens`]: (request: { body?: string | null }) => {
        const name = (JSON.parse(request.body ?? "{}") as { name: string }).name;
        if (name === "taken") return { status: 409, body: { error: "taken" } };
        screens.push(name);
        return { status: 201, body: { id: `s-${screens.length - 1}` } };
      },
    },
  };
}

test("the dashboard loads its metrics over the stubbed API", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:Sessions today");
  expect(app.texts()).toContain("1,284");
  expect(app.texts()).toContain("Lobby board");
});

test("refresh reloads the dashboard", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:Sessions today");
  await app.press("role:button[Refresh]");
  await app.waitFor("text:Sessions today");
  expect(app.requests().filter((r) => r.method === "GET").length).toBe(2);
});

test("a failed load shows an error and Retry recovers", async () => {
  let calls = 0;
  const app = await open("./App.tsx", {
    clock: {},
    network: {
      [`GET ${API}/metrics`]: () => {
        calls += 1;
        return calls === 1
          ? { error: "offline" }
          : { body: [{ id: "m-recovered", label: "Recovered", value: "1" }] };
      },
    },
  });
  await app.waitFor("text:/Couldn't load the dashboard/");
  expect(app.texts()).not.toContain("Recovered");
  await app.press("role:button[Retry]");
  await app.waitFor("text:Recovered");
});

test("an empty name stays unregistered and shows the rule error", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:Sessions today");
  await app.press("role:button[Register]");
  await app.waitFor("text:Name the screen");
  expect(app.requests().filter((r) => r.method === "POST").length).toBe(0);
});

test("a name the server already has lands on the field", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:Sessions today");
  await app.type("id:name", "taken");
  await app.press("role:button[Register]");
  await app.waitFor("text:That name is already registered");
});

test("registering a screen invalidates the dashboard", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:Sessions today");
  await app.type("id:name", "Wall board");
  await app.press("role:button[Register]");
  await app.waitFor("text:Registered as s-1");
  await app.clear("id:name");
  await app.waitFor("text:Wall board");
});

test("pinging the host answers through the host channel", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", {
    clock: {},
    network: api.routes,
    host: { handlers: { "remote.ping": async ({ at }: { at: number }) => ({ at }) } },
  });
  await app.waitFor("text:Embedded in a host app");
  await app.press("role:button[Ping host]");
  await app.waitFor("text:/^Pong in/");
  expect(app.host.calls()[0]?.name).toBe("remote.ping");
});

test("a host without a handler for the name rejects the ping", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", {
    clock: {},
    network: api.routes,
    host: { handlers: { "other.thing": null } },
  });
  await app.press("role:button[Ping host]");
  await app.waitFor("text:Ping failed: ERR_NSH_HOST_ERROR");
});

test("no host at all is a different failure", async () => {
  const api = dashboardApi();
  const app = await open("./App.tsx", { clock: {}, network: api.routes });
  await app.waitFor("text:No host on this platform");
  await app.press("role:button[Ping host]");
  await app.waitFor("text:Ping failed: ERR_FEATURE_UNAVAILABLE_ON_PLATFORM");
});
