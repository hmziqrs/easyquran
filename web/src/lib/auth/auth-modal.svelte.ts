export type AuthMode = "login" | "register";

class AuthModal {
  open = $state(false);
  mode = $state<AuthMode>("login");

  show(mode: AuthMode = "login"): void {
    this.mode = mode;
    this.open = true;
  }

  close(): void {
    this.open = false;
  }
}

export const authModal = new AuthModal();
