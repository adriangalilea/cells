// A SPAN: two instants, `from` inclusive, `to` exclusive, whole ms on a media clock;
// geometry and nothing more. What a scroll stage's act cues
// (ui/playback `cue`), what a timebase loops in, where a division runs, what a claim
// is about, what an effect targets. Framework-free, and its own item so the many
// things that speak of a stretch of time import it without the rest of a playable.

/** A stretch of time, ms: `from` inclusive, `to` exclusive. */
export interface Span {
  from: number
  to: number
}
