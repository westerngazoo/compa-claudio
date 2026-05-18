import type { Personality } from "./personalities";

/**
 * Mascot — El Compa Claudio's body and idle behaviors.
 * Blink + breathing are CSS-driven; this class handles the stateful moments:
 * the thought bubble, hover excitement, and swapping hats when his
 * personality changes.
 */
export class Mascot {
  private el: HTMLElement;
  private bubble: HTMLElement;
  private hatSlot: SVGGElement | null;
  private bubbleVisible = false;

  constructor(root: HTMLElement) {
    this.el = root.querySelector<HTMLElement>("#mascot")!;
    this.bubble = root.querySelector<HTMLElement>("#thought-bubble")!;
    this.hatSlot = root.querySelector<SVGGElement>("#hat-slot");

    this.el.addEventListener("mouseenter", () => this.setExcited(true));
    this.el.addEventListener("mouseleave", () => this.setExcited(false));
  }

  setExcited(on: boolean) {
    this.el.classList.toggle("excited", on);
  }

  /** Swap Claudio's hat to match a personality, with a little pop. */
  setPersonality(p: Personality) {
    if (!this.hatSlot) return;
    this.hatSlot.innerHTML = p.hatSvg;
    // Restart the pop animation: remove the class, force a reflow, re-add.
    this.hatSlot.classList.remove("hat-pop");
    void this.hatSlot.getBoundingClientRect();
    if (p.hatSvg.trim()) {
      this.hatSlot.classList.add("hat-pop");
    }
  }

  showThought() {
    if (this.bubbleVisible) return;
    this.bubbleVisible = true;
    this.bubble.classList.remove("hidden");
    requestAnimationFrame(() => this.bubble.classList.add("visible"));
  }

  hideThought() {
    if (!this.bubbleVisible) return;
    this.bubbleVisible = false;
    this.bubble.classList.remove("visible");
    setTimeout(() => this.bubble.classList.add("hidden"), 220);
  }

  onClick(handler: () => void) {
    this.el.addEventListener("dblclick", handler);
  }

  onThoughtClick(handler: () => void) {
    this.bubble.addEventListener("click", (e) => {
      e.stopPropagation();
      handler();
    });
  }
}
