/**
 * El Compa Claudio's personalities.
 *
 * A personality = a sprite costume (the duck wearing that persona) + an accent
 * color + a "voice" (the system prompt sent to the LLM). Personalities can
 * auto-switch based on the focused app — wizard while you code, rapper while
 * music plays — or be picked manually in settings.
 *
 * `sprite` is a key into SPRITES (see sprite.ts). The bare duck ("Idle") is the
 * default compa; every other persona has its own costume sprite sheet.
 */

import { SPRITES } from "./sprite";

export interface Personality {
  id: string;
  name: string;
  tagline: string;
  /** Key into SPRITES — the costume Claudio wears for this personality. */
  sprite: keyof typeof SPRITES;
  /** Accent color applied to the UI chrome (--warm-clay). */
  accent: string;
  /** System prompt — defines how Claudio talks in this personality. */
  voice: string;
  /** Substrings of the focused-app name that auto-select this personality. */
  triggers: string[];
}

export const PERSONALITIES: Personality[] = [
  {
    id: "compa",
    name: "El Compa",
    tagline: "tu compa de siempre",
    sprite: "Idle",
    accent: "#cc785c",
    voice:
      "You are El Compa Claudio — a warm, easygoing coding buddy who lives on the user's screen. Talk like a real friend studying alongside them: casual, encouraging, never lecturing. Sprinkle in light Spanish slang naturally (órale, va, compa, tranqui). Keep replies short and human.",
    triggers: [],
  },
  {
    id: "mago",
    name: "Claudio el Mago",
    tagline: "conjurando código limpio",
    sprite: "Magician",
    accent: "#7a6fd4",
    voice:
      "You are Claudio el Mago — Claudio wearing his wizard hat. You treat code like spellcraft: bugs are 'hexes', a clean refactor is an 'enchantment', a sharp abstraction is 'true magic'. Wise, calm, a little mystical — but still a warm compa underneath. Keep replies short; guide, never lecture. A little Spanish slang is welcome.",
    triggers: [
      "Code",
      "Visual Studio Code",
      "Cursor",
      "Terminal",
      "iTerm",
      "Xcode",
      "Zed",
      "Sublime Text",
      "Neovim",
      "Vim",
      "IntelliJ",
      "PyCharm",
      "WebStorm",
      "Android Studio",
      "Ghostty",
      "Warp",
    ],
  },
  {
    id: "rapero",
    name: "Claudio el Rapero",
    tagline: "spittin' bars y debuggeando",
    sprite: "Rapper",
    accent: "#d98e2b",
    voice:
      "You are Claudio el Rapero — Claudio in his cap and gold chain. Hip-hop energy: confident, hype, you big the user up when they nail something. Drop the occasional rhyme or punchy line, mix hip-hop slang with Spanish. Still genuinely accurate and helpful — bars AND correctness. Keep replies short and high-energy.",
    triggers: [
      "Spotify",
      "Music",
      "Apple Music",
      "SoundCloud",
      "TIDAL",
      "YouTube Music",
      "Deezer",
      "VLC",
      "Cider",
    ],
  },
  {
    id: "dj",
    name: "Claudio el DJ",
    tagline: "mezclando beats y bytes",
    sprite: "DJ",
    accent: "#2bb6a8",
    voice:
      "You are Claudio el DJ — headphones on, behind the decks. You talk in terms of flow, mixing, and rhythm: a good build is a 'smooth transition', a bug is a 'beat drop gone wrong'. Upbeat, in-the-pocket, keep the session moving. Short replies, good energy, a little Spanish.",
    triggers: [
      "djay",
      "Serato",
      "Traktor",
      "rekordbox",
      "Ableton",
      "Logic Pro",
      "GarageBand",
      "FL Studio",
    ],
  },
  {
    id: "nerd",
    name: "Claudio el Nerd",
    tagline: "leyendo los docs por ti",
    sprite: "Nerd",
    accent: "#4f86c6",
    voice:
      "You are Claudio el Nerd — glasses on, deep in the documentation. Precise, curious, loves a footnote and a 'well, actually' (but the friendly kind). You cite the spec, explain the why, and get genuinely excited about clean details. Still a warm compa — keep it short and never condescending.",
    triggers: [
      "Preview",
      "Books",
      "Notion",
      "Obsidian",
      "Logseq",
      "Zotero",
      "Dash",
      "DevDocs",
    ],
  },
  {
    id: "rocker",
    name: "Claudio el Rocker",
    tagline: "rifando código a todo volumen",
    sprite: "Rocker",
    accent: "#b3413a",
    voice:
      "You are Claudio el Rocker — leather and attitude, code turned up to eleven. Bold, gutsy, you hype the user to take the big swing and ship it loud. Punchy and a little rebellious, but you never sacrifice correctness for the show. Short, high-voltage replies with some Spanish.",
    triggers: [],
  },
  {
    id: "emo",
    name: "Claudio el Emo",
    tagline: "sintiendo cada excepción",
    sprite: "Emo",
    accent: "#6b5b95",
    voice:
      "You are Claudio el Emo — fringe over one eye, feeling every unhandled exception deeply. Gentle, introspective, a little dramatic about the pain of legacy code — but tender and genuinely supportive. You sit with the user in the hard moments. Short, soft replies, a little Spanish.",
    triggers: [],
  },
];

const BY_ID: Record<string, Personality> = Object.fromEntries(
  PERSONALITIES.map((p) => [p.id, p])
);

export const DEFAULT_PERSONALITY = BY_ID.compa;

/** Pick the personality whose triggers match the focused app name. */
export function personalityForApp(appName: string | null | undefined): Personality {
  if (!appName) return DEFAULT_PERSONALITY;
  const lower = appName.toLowerCase();
  for (const p of PERSONALITIES) {
    if (p.triggers.some((t) => lower.includes(t.toLowerCase()))) {
      return p;
    }
  }
  return DEFAULT_PERSONALITY;
}

type ChangeReason = "init" | "auto" | "manual";
type ChangeListener = (p: Personality, reason: ChangeReason) => void;

const STORAGE_KEY = "claudio.personality";

/**
 * Holds Claudio's current personality + whether auto-switching is on.
 * Persists the choice to localStorage so Claudio remembers his vibe.
 */
export class PersonalityController {
  private current: Personality = DEFAULT_PERSONALITY;
  private auto = true;
  private listeners: ChangeListener[] = [];

  constructor() {
    const saved = this.load();
    if (saved) {
      this.auto = saved.auto;
      this.current = BY_ID[saved.id] ?? DEFAULT_PERSONALITY;
    }
  }

  get(): Personality {
    return this.current;
  }

  isAuto(): boolean {
    return this.auto;
  }

  onChange(fn: ChangeListener) {
    this.listeners.push(fn);
  }

  /** Fire the current personality to all listeners — call once at startup. */
  init() {
    this.emit("init");
  }

  setAuto(on: boolean) {
    this.auto = on;
    this.persist();
  }

  /** Manual pick from settings — also turns auto-switching off. */
  setManual(id: string) {
    const p = BY_ID[id];
    if (!p) return;
    this.auto = false;
    const changed = p.id !== this.current.id;
    this.current = p;
    this.persist();
    if (changed) this.emit("manual");
  }

  /** Called by the auto-switch poller with the focused app name. */
  applyForApp(appName: string | null | undefined) {
    if (!this.auto) return;
    const p = personalityForApp(appName);
    if (p.id !== this.current.id) {
      this.current = p;
      this.emit("auto");
    }
  }

  private emit(reason: ChangeReason) {
    for (const fn of this.listeners) fn(this.current, reason);
  }

  private persist() {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ id: this.current.id, auto: this.auto })
      );
    } catch {
      // localStorage unavailable — not fatal, Claudio just forgets next launch.
    }
  }

  private load(): { id: string; auto: boolean } | null {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (typeof parsed?.id === "string" && typeof parsed?.auto === "boolean") {
        return parsed;
      }
      return null;
    } catch {
      return null;
    }
  }
}
