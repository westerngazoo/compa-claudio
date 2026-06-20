import type { Personality } from "./personalities";
import { SpriteAnimator } from "./sprite";

/**
 * Mascot — El Compa Claudio (the duck) and his idle behaviors.
 *
 * The body is now a sprite animation (see sprite.ts) instead of inline SVG.
 * This class owns the SpriteAnimator and the stateful moments around it:
 * swapping persona costumes, the talking loop, one-shot reactions, the thought
 * bubble, and hover excitement.
 */
export class Mascot {
  private el: HTMLElement;
  private duck: HTMLElement;
  private bubble: HTMLElement;
  private sprite: SpriteAnimator;
  private bubbleVisible = false;
  private talking = false;
  private idleSprite = "Idle";

  constructor(root: HTMLElement) {
    this.el = root.querySelector<HTMLElement>("#mascot")!;
    this.bubble = root.querySelector<HTMLElement>("#thought-bubble")!;
    this.duck = root.querySelector<HTMLElement>("#duck-sprite")!;
    this.sprite = new SpriteAnimator(this.duck);

    this.el.addEventListener("mouseenter", () => this.setExcited(true));
    this.el.addEventListener("mouseleave", () => this.setExcited(false));
  }

  setExcited(on: boolean) {
    this.el.classList.toggle("excited", on);
  }

  /** Swap Claudio's costume to match a personality. */
  setPersonality(p: Personality) {
    this.idleSprite = p.sprite;
    this.sprite.setIdle(p.sprite);
    // A CSS hop sells the costume change without dropping the sprite sheet —
    // works for every persona, not just the bare duck.
    this.hop();
  }

  /** Drive the talking loop while Claudio is replying. */
  setTalking(on: boolean) {
    if (this.talking === on) return;
    this.talking = on;
    this.el.classList.toggle("talking", on);
    // Only the bare duck has a dedicated Talking strip. Costumed personas keep
    // their costume loop (so the costume never blinks away) and lean on the
    // CSS bob from the `.talking` class instead.
    if (on && this.idleSprite === "Idle") {
      this.sprite.loopWith("Talking");
    } else if (!on && this.idleSprite === "Idle") {
      this.sprite.rest();
    }
  }

  private hop() {
    this.duck.classList.remove("hop");
    void this.duck.getBoundingClientRect(); // force reflow to restart the anim
    this.duck.classList.add("hop");
  }

  /** Play a one-shot reaction (Happy, Surprised, Jumping, …) then return idle. */
  react(action: string) {
    if (this.talking) return;
    this.sprite.react(action);
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
