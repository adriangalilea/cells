// What the product is called, once: every page, title and command reads it here, so a
// rename is one edit.

export const PRODUCT = {
  /** The format, the project and the app; a file is a Cells file, and a cell only ever a
   *  table's. */
  name: "Cells",
  /** A file's extension, with its dot: the format's own name. */
  extension: ".cells",
  /** The command line. */
  cli: "cells",
  tagline: "Programmable data.",
  /** Under the line, and the page's description wherever it is read; on the page its
   *  extension is lit. */
  subline: "Plain text .cells files: typed, reactive, composable and charted.",
  domain: "cellular.computer",
  repo: "https://github.com/adriangalilea/cells",
} as const
