// The themes the showcase shows, each a dark and a light pair, the first
// shown first. All are bundled with Shiki (from tm-themes) and loaded by
// name; none is copied here. Each is MIT, by its own repository's licence.
export const PAIRS = [
  // github.com/rose-pine/vscode
  { id: "rose-pine", label: "Rosé Pine", dark: "rose-pine-moon", light: "rose-pine-dawn" },
  // github.com/primer/github-vscode-theme
  { id: "github", label: "GitHub", dark: "github-dark", light: "github-light" },
  // github.com/Binaryify/OneDark-Pro, github.com/akamud/vscode-theme-onelight
  { id: "one", label: "One", dark: "one-dark-pro", light: "one-light" },
  // github.com/catppuccin/vscode
  { id: "catppuccin", label: "Catppuccin", dark: "catppuccin-mocha", light: "catppuccin-latte" },
] as const
