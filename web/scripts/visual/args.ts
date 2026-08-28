/**
 * Minimal repeatable-flag parser for the visual harness CLIs. No dependencies: the
 * harness must stay runnable with `node scripts/visual/<script>.ts` alone.
 *
 *   --tier phase            → flag("tier") === "phase"
 *   --route /design/tokens  → list("route") === ["/design/tokens"]  (repeatable)
 *   --report                → has("report") === true
 */

export class Args {
  #single = new Map<string, string>();
  #multi = new Map<string, string[]>();
  #flags = new Set<string>();

  constructor(argv: string[]) {
    for (let i = 0; i < argv.length; i += 1) {
      const arg = argv[i]!;
      if (!arg.startsWith("--")) continue;
      const key = arg.slice(2);
      const value = argv[i + 1];
      if (value !== undefined && !value.startsWith("--")) {
        this.#single.set(key, value);
        const multi = this.#multi.get(key) ?? [];
        multi.push(value);
        this.#multi.set(key, multi);
        i += 1;
      } else {
        this.#flags.add(key);
      }
    }
  }

  flag(name: string, fallback: string): string {
    return this.#single.get(name) ?? fallback;
  }

  list(name: string): string[] {
    return this.#multi.get(name) ?? [];
  }

  has(name: string): boolean {
    return this.#flags.has(name);
  }
}
