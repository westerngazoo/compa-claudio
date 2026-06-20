/**
 * Sprite animation engine for El Compa Claudio (the duck).
 *
 * Each animation is a horizontal strip PNG of N frames, every frame
 * `FRAME_W × FRAME_H`. We render one frame at a time into a fixed-size element
 * via `background-image` + `background-position-x`, stepping frames on a timer.
 *
 * Metadata mirrors public/sprites/atlas.json (kept in sync by hand — it's tiny
 * and stable). The master sheet in design/character/ is reference-only.
 */

export const FRAME_W = 240;
export const FRAME_H = 288;

export interface SpriteDef {
  /** Filename under /sprites/. */
  file: string;
  /** Number of frames in the strip. */
  frames: number;
  /** Playback frames-per-second. */
  fps: number;
  /** Whether the animation loops (idle/talking) or plays once (reactions). */
  loop: boolean;
}

/** Every animation available, keyed by a stable id. */
export const SPRITES: Record<string, SpriteDef> = {
  // --- base-duck actions ---
  Idle: { file: "action-Idle.png", frames: 3, fps: 4, loop: true },
  Talking: { file: "action-Talking.png", frames: 3, fps: 9, loop: true },
  Jumping: { file: "action-Jumping.png", frames: 4, fps: 12, loop: false },
  Happy: { file: "action-Happy.png", frames: 3, fps: 8, loop: false },
  Sad: { file: "action-Sad.png", frames: 2, fps: 3, loop: false },
  Walk_Run: { file: "action-Walk_Run.png", frames: 4, fps: 10, loop: true },
  Dancing: { file: "action-Dancing.png", frames: 4, fps: 10, loop: true },
  Surprised: { file: "action-Surprised.png", frames: 3, fps: 10, loop: false },
  Angry: { file: "action-Angry.png", frames: 3, fps: 8, loop: false },
  // --- persona costumes (idle loops) ---
  Rapper: { file: "persona-Rapper.png", frames: 3, fps: 4, loop: true },
  Rocker: { file: "persona-Rocker.png", frames: 3, fps: 4, loop: true },
  Magician: { file: "persona-Magician.png", frames: 3, fps: 4, loop: true },
  Nerd: { file: "persona-Nerd.png", frames: 2, fps: 3, loop: true },
  Emo: { file: "persona-Emo.png", frames: 2, fps: 3, loop: true },
  DJ: { file: "persona-DJ.png", frames: 2, fps: 4, loop: true },
};

const SPRITE_BASE = "/sprites/";

/**
 * Drives a single DOM element through sprite animations.
 *
 * Two layers of state:
 * - `idle`: the looping animation Claudio returns to (his persona costume, or
 *   "Idle" for the bare duck).
 * - a transient one-shot reaction (Happy, Surprised, …) that plays once and
 *   then falls back to `idle`.
 */
export class SpriteAnimator {
  private el: HTMLElement;
  private idleId = "Idle";
  private playingId = "Idle";
  private frame = 0;
  private timer: number | null = null;
  private preloaded = new Set<string>();

  constructor(el: HTMLElement) {
    this.el = el;
    this.el.style.width = `${FRAME_W}px`;
    this.el.style.height = `${FRAME_H}px`;
    this.el.style.backgroundRepeat = "no-repeat";
    this.preload("Idle");
    this.play("Idle");
  }

  /** Preload a sheet so the first swap doesn't flash an empty frame. */
  preload(id: string) {
    const def = SPRITES[id];
    if (!def || this.preloaded.has(id)) return;
    const img = new Image();
    img.src = SPRITE_BASE + def.file;
    this.preloaded.add(id);
  }

  /** Set the looping idle/persona animation Claudio rests in. */
  setIdle(id: string) {
    if (!SPRITES[id]) return;
    this.idleId = id;
    this.preload(id);
    // If we're not mid-reaction, switch to the new idle immediately.
    if (SPRITES[this.playingId]?.loop) {
      this.play(id);
    }
  }

  /** Play a one-shot reaction, then fall back to the current idle. */
  react(id: string) {
    if (!SPRITES[id]) return;
    this.preload(id);
    this.play(id);
  }

  /** Swap to a looping animation (e.g. Talking) and stay there until changed. */
  loopWith(id: string) {
    if (!SPRITES[id]) return;
    this.preload(id);
    this.play(id);
  }

  /** Return to the resting idle/persona loop. */
  rest() {
    this.play(this.idleId);
  }

  private play(id: string) {
    const def = SPRITES[id];
    if (!def) return;
    this.playingId = id;
    this.frame = 0;

    const totalW = def.frames * FRAME_W;
    this.el.style.backgroundImage = `url(${SPRITE_BASE}${def.file})`;
    this.el.style.backgroundSize = `${totalW}px ${FRAME_H}px`;
    this.el.style.backgroundPositionX = "0px";

    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
    if (def.frames <= 1) return;

    this.timer = window.setInterval(() => this.tick(def), 1000 / def.fps);
  }

  private tick(def: SpriteDef) {
    this.frame += 1;
    if (this.frame >= def.frames) {
      if (def.loop) {
        this.frame = 0;
      } else {
        // One-shot finished — hold the last frame briefly, then rest.
        if (this.timer !== null) {
          clearInterval(this.timer);
          this.timer = null;
        }
        window.setTimeout(() => this.rest(), 120);
        return;
      }
    }
    this.el.style.backgroundPositionX = `-${this.frame * FRAME_W}px`;
  }

  destroy() {
    if (this.timer !== null) clearInterval(this.timer);
  }
}
