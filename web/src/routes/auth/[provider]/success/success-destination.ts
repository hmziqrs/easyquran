export interface SuccessRouteUser {
  readonly is_verified: boolean;
}

export function successDestination(
  returnTarget: string | null,
  user: SuccessRouteUser | null,
): string {
  const unverified = user ? !user.is_verified : false;
  return returnTarget ?? (unverified ? "/verify-email" : "/app");
}
