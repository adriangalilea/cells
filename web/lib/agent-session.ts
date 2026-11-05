// A coding agent's session as data: what the person asked, what the agent said, the
// tools it ran and what came back, and whether it is still working. The claude-code
// and codex skins draw the same entries in their own terminal UIs; a story (a macos
// timeline) or a page supplies them. Framework-free by contract.

export type AgentEntry =
  | { kind: "prompt"; text: string }
  | { kind: "say"; text: string }
  /** A tool call and its result, one line each. `result` empty while it runs. */
  | { kind: "tool"; name: string; arg: string; result: string[] }

/** The agent at work. Facts only; each skin words and draws its own working line. */
export interface AgentWork {
  /** How long the turn has run, in the world's seconds (a shut lid's hours count). */
  seconds: number
  /** How long the request in flight has run, in the world's seconds: the model is
   *  asked again after every reply and tool result, and a request thinks first. */
  request: number
  /** Output tokens the turn has produced so far; 0 until its first output. */
  tokens: number
  /** Time on screen since the turn started, in ms: the spinner and the shimmer run
   *  on it, so a frame drawn alone animates exactly like one played to. */
  ms: number
}

/** How long a request thinks before it streams output, in seconds. */
export const THINK_SECONDS = 30
/** Output tokens a request streams per second once it is past thinking. */
const STREAM_PER_SECOND = 90

/** The output tokens an entry the agent wrote cost: the thinking behind it and its
 *  words, at about four characters a token. A prompt is the person's, not output. */
export function entryTokens(e: AgentEntry): number {
  if (e.kind === "prompt") return 0
  const chars =
    e.kind === "say" ? e.text.length : e.arg.length + e.result.join(" ").length
  return 320 + Math.round(chars / 4)
}

/** Output tokens of the request in flight, `seconds` old: none while it thinks,
 *  then streaming. */
export function requestTokens(seconds: number): number {
  return Math.round(Math.max(0, seconds - THINK_SECONDS) * STREAM_PER_SECOND)
}

/** The turn over: how long it took, in the world's seconds. Each skin draws its own
 *  closing line ("✻ Sautéed for 8m 37s"). */
export interface AgentFinished {
  seconds: number
}

/** "1.5k": a token count the way Claude Code prints it. */
export function formatTokens(n: number): string {
  return n < 1000 ? `${n}` : `${(n / 1000).toFixed(1)}k`
}

/** What a skin draws: the transcript so far, the working line if any, and what is
 *  being typed into the prompt box. */
export interface AgentViewProps {
  entries: AgentEntry[]
  work: AgentWork | null
  /** The last turn's closing line, once it is done; null while working or idle. */
  finished?: AgentFinished | null
  draft: string
  /** The working directory the session was started in, shown in the header. */
  cwd?: string
  className?: string
}

/** "2h 14m", "3m 07s", "12s": how both CLIs say elapsed time, near enough. */
export function formatElapsed(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${String(s % 60).padStart(2, "0")}s`
  return `${s}s`
}

// Runnable example: the claude-code and codex demos (the lib carries no runtime).
