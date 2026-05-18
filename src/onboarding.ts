import { invoke } from "@tauri-apps/api/core";

type Status = "trusted" | "notTrusted" | "notApplicable";

/**
 * First-run accessibility permission flow.
 * - Shows a friendly modal explaining why we need AX access (macOS only).
 * - Triggers macOS' system prompt when the user clicks "let me see."
 * - Polls in the background while open; if the user grants access in
 *   System Settings, the modal flips to a "thanks!" state and auto-closes.
 */
export class Onboarding {
  private el: HTMLElement;
  private hint: HTMLElement;
  private grantBtn: HTMLButtonElement;
  private dismissBtn: HTMLButtonElement;
  private poll: number | null = null;
  private granted = false;

  constructor(root: HTMLElement) {
    this.el = root.querySelector<HTMLElement>("#onboarding")!;
    this.hint = root.querySelector<HTMLElement>(".onboarding-hint")!;
    this.grantBtn = root.querySelector<HTMLButtonElement>("#onboarding-grant")!;
    this.dismissBtn = root.querySelector<HTMLButtonElement>("#onboarding-dismiss")!;

    this.grantBtn.addEventListener("click", () => void this.requestAccess());
    this.dismissBtn.addEventListener("click", () => this.hide());
  }

  /**
   * Check current AX status; show the modal if `notTrusted`.
   * Silent no-op on Linux/Windows (status `notApplicable`).
   */
  async maybeShow() {
    try {
      const status = await invoke<Status>("accessibility_status");
      if (status === "notTrusted") this.show();
    } catch {
      // Tauri not available (e.g. plain Vite preview) — silently skip.
    }
  }

  private show() {
    this.el.classList.remove("hidden");
    this.startPolling();
  }

  hide() {
    this.el.classList.add("hidden");
    this.stopPolling();
  }

  private async requestAccess() {
    try {
      await invoke<Status>("request_accessibility");
      this.hint.textContent =
        "macOS asked. find me in System Settings → Privacy & Security → Accessibility and toggle me on.";
      this.grantBtn.textContent = "open settings…";
      this.grantBtn.classList.remove("primary");
      // The polling loop below will catch the grant and close the modal.
    } catch (e) {
      this.hint.textContent = `couldn't prompt: ${e}`;
    }
  }

  private startPolling() {
    if (this.poll !== null) return;
    this.poll = window.setInterval(async () => {
      try {
        const status = await invoke<Status>("accessibility_status");
        if (status === "trusted" && !this.granted) {
          this.markGranted();
        }
      } catch {
        // ignore
      }
    }, 1200);
  }

  private stopPolling() {
    if (this.poll !== null) {
      window.clearInterval(this.poll);
      this.poll = null;
    }
  }

  private markGranted() {
    this.granted = true;
    this.el.classList.add("granted");
    const h2 = this.el.querySelector<HTMLElement>("h2");
    if (h2) h2.textContent = "thanks — i can see you now";
    this.hint.textContent = "closing in a sec…";
    this.grantBtn.style.display = "none";
    this.dismissBtn.textContent = "let's go";
    setTimeout(() => this.hide(), 1800);
  }
}
