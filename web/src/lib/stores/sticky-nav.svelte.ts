class StickyNav {
  collapsed = $state(false);
  readonly height = 56;

  collapse(): void {
    this.collapsed = true;
  }
  expand(): void {
    this.collapsed = false;
  }
}

export const stickyNav = new StickyNav();
