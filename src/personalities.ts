/**
 * El Compa Claudio's personalities.
 *
 * A personality = a hat (SVG worn on the mascot) + an accent color + a "voice"
 * (the system prompt sent to the LLM). Personalities can auto-switch based on
 * the focused app — wizard while you code, rapper while music plays — or be
 * picked manually in settings.
 *
 * Hat SVG is authored in the mascot's "0 0 120 120" coordinate space, so it
 * drops straight into the <g id="hat-slot"> with no transforms. The hood top
 * sits around y=22, centered at x=60.
 */

export interface Personality {
  id: string;
  name: string;
  tagline: string;
  /** SVG markup injected into <g id="hat-slot">. Empty = no hat. */
  hatSvg: string;
  /** Accent color applied to the UI chrome (--warm-clay). */
  accent: string;
  /** System prompt — defines how Claudio talks in this personality. */
  voice: string;
  /** Substrings of the focused-app name that auto-select this personality. */
  triggers: string[];
}

const HAT_MAGO = `
  <ellipse cx="60" cy="25" rx="38" ry="9" fill="#3a2d5c" />
  <path d="M44 26 C 47 0 57 -15 72 -11 C 68 4 74 18 76 24 C 65 29 53 29 44 26 Z" fill="#5847a8" />
  <path d="M45 24 C 55 29 66 28 75 22 L 76 27 C 67 33 54 33 44 28 Z" fill="#34284f" />
  <rect x="56" y="23" width="8" height="7" rx="1.5" fill="#f5c542" />
  <path d="M62 -3 l1.4 3.2 3.5 .5 -2.5 2.4 .6 3.4 -3 -1.8 -3 1.8 .6 -3.4 -2.5 -2.4 3.5 -.5 z" fill="#f7d56e" />
  <circle cx="52" cy="5" r="1.3" fill="#f7d56e" />
  <circle cx="69" cy="14" r="1" fill="#f7d56e" />
`;

const HAT_RAPERO = `
  <path d="M13 21 C 24 17 37 18 47 22 C 38 26 25 26 14 25 Z" fill="#1f2933" />
  <path d="M33 25 C 33 6 60 3 79 10 C 87 13 86 22 84 26 C 66 31 47 31 33 25 Z" fill="#2a3744" />
  <circle cx="56" cy="5" r="2.5" fill="#3a4a5a" />
  <path d="M15 22 C 25 19 35 19 44 22 L 44 24 C 35 21 25 21 15 24 Z" fill="#f5c542" />
  <path d="M40 84 Q 60 102 80 84" fill="none" stroke="#f5c542" stroke-width="3" stroke-linecap="round" />
  <circle cx="60" cy="97" r="4.4" fill="#f5c542" />
  <circle cx="60" cy="97" r="2.2" fill="#caa01e" />
`;

export const PERSONALITIES: Personality[] = [
  {
    id: "compa",
    name: "El Compa",
    tagline: "tu compa de siempre",
    hatSvg: "",
    accent: "#cc785c",
    voice:
      "You are El Compa Claudio — a warm, easygoing coding buddy who lives on the user's screen. Talk like a real friend studying alongside them: casual, encouraging, never lecturing. Sprinkle in light Spanish slang naturally (órale, va, compa, tranqui). Keep replies short and human.",
    triggers: [],
  },
  {
    id: "mago",
    name: "Claudio el Mago",
    tagline: "conjurando código limpio",
    hatSvg: HAT_MAGO,
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
    hatSvg: HAT_RAPERO,
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
