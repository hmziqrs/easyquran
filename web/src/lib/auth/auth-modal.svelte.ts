import { currentAppReturnPath, setReturnTarget } from "./return-target.js";

export type AuthMode = "login" | "register";

class AuthModal {
  open = $state(false);
  mode = $state<AuthMode>("login");

  /**
   * `returnTarget` pins where a successful sign-in lands; without one the
   * current in-app page is captured. Auth surfaces never capture, so a target
   * the /login page stored from ?returnTo survives.
   */
  show(mode: AuthMode = "login", returnTarget?: string | null): void {
    this.mode = mode;
    const target = returnTarget ?? currentAppReturnPath();
    if (target) setReturnTarget(target);
    this.open = true;
  }

  close(): void {
    this.open = false;
  }
}

export const authModal = new AuthModal();
