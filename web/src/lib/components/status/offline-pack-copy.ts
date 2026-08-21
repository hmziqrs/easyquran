export interface OfflinePackCopy {
  readonly heading: string;
  readonly routes: (entries: number, size: string) => string;
  readonly saved: (when: string) => string;
  readonly usage: (used: string, quota: string) => string;
  readonly toggleOn: string;
  readonly toggleOff: string;
  readonly busy: string;
  readonly retry: string;
}
