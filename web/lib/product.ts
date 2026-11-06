// What the product is called, once: every page, title and command reads it here, so a
// rename is one edit.

export const PRODUCT = {
  /** The format, the project and the app. */
  name: "Cells",
  /** What one file is called. */
  file: "cell",
  /** A file's extension, with its dot. */
  extension: ".cell",
  /** The command line. */
  cli: "cells",
  tagline: "Programmable data.",
  /** Under the line, and the page's description wherever it is read; on the page its
   *  extension is lit. */
  subline: "Plain text .cell files: typed, reactive and composable.",
  domain: "cellular.computer",
  repo: "https://github.com/adriangalilea/cells",
} as const
