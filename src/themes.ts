export const themes = [
  { id: 'rose', name: 'Розовый сумрак' },
  { id: 'purple', name: 'Классический фиолетовый' },
  { id: 'graphite', name: 'Графит' },
  { id: 'nord', name: 'Nord' },
  { id: 'dracula', name: 'Dracula' },
  { id: 'catppuccin', name: 'Catppuccin Mocha' },
  { id: 'tokyo', name: 'Tokyo Night' },
  { id: 'rosepine', name: 'Rosé Pine' },
  { id: 'gruvbox', name: 'Gruvbox' },
  { id: 'eastbay', name: 'East Bay' },
  { id: 'coastal', name: 'Coastal Blue' },
  { id: 'charcoal', name: 'Charcoal & Ivory' },
] as const;
export type Theme = typeof themes[number]['id'];
