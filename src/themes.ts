export const themes = [
  { id: 'rose', name: 'Розовый сумрак', description: 'Тёплый розовый. Глубокий фиолетовый. Твой тихий вечер.' },
  { id: 'purple', name: 'Классический фиолетовый', description: 'Тот самый Midnight — в оттенках ночного неба.' },
  { id: 'graphite', name: 'Графит', description: 'Прохладный графит и серебристые акценты.' },
  { id: 'nord', name: 'Nord', description: 'Арктические оттенки для спокойных приключений.' },
  { id: 'dracula', name: 'Dracula', description: 'Насыщенный фиолетовый с характером.' },
  { id: 'catppuccin', name: 'Catppuccin Mocha', description: 'Мягкая пастель и немного вечернего уюта.' },
  { id: 'tokyo', name: 'Tokyo Night', description: 'Синие огни города, который ещё не спит.' },
  { id: 'rosepine', name: 'Rosé Pine', description: 'Приглушённые краски и пыльная роза.' },
  { id: 'gruvbox', name: 'Gruvbox', description: 'Тёплые землистые тона с золотым акцентом.' },
  { id: 'eastbay', name: 'East Bay', description: 'Глубокий индиго и мягкий свет лаванды.' },
  { id: 'coastal', name: 'Coastal Blue', description: 'Свежий морской воздух в синих оттенках.' },
  { id: 'charcoal', name: 'Charcoal & Ivory', description: 'Угольный фон. Свет слоновой кости. Ничего лишнего.' },
] as const;
export type Theme = typeof themes[number]['id'];
