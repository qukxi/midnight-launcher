import type { Language } from './model';
export const english: Record<string, string> = {
  'Восстановить сохранённый вход': 'Restore saved sign-in',
  'Консоль игры': 'Game console', 'Вывод запуска и ошибки Minecraft': 'Minecraft startup output and errors',
  'Сборка для просмотра логов': 'Instance logs', 'Обновляется автоматически': 'Updates automatically',

  'Выбрать сборку': 'Choose an instance', 'Сменить': 'Change', 'Управление сборками': 'Manage instances',

  'Поведение лаунчера': 'Launcher behavior', 'После запуска игры': 'After game launch',
  'Оставлять открытым': 'Keep open', 'Сворачивать лаунчер': 'Minimize launcher',
  'Возвращать окно после выхода из игры': 'Restore window after the game exits',
  'Открыть логи': 'Open logs', 'Настроить сборку': 'Customize instance',
  'Сохранить сборку': 'Save instance', 'Память сборки, МБ': 'Instance memory, MB',
  'Как в настройках': 'Use global settings', 'Java для сборки': 'Instance Java', 'Цвет карточки': 'Card color',
  'Укажите память от 1024 до 16384 МБ.': 'Enter memory between 1024 and 16384 MB.',
  'Сборка сохранена. Недостающие файлы установятся при запуске.': 'Instance saved. Missing files will be installed on launch.',

  'Введите версию вручную или обновите список.': 'Enter a version or refresh the list.',
  'Получите список версий или введите вручную.': 'Fetch versions or enter one manually.',

  'Загрузчик': 'Loader', 'позже': 'coming later', 'Версия загрузчика': 'Loader version',
  'Выберите версию Fabric.': 'Select a Fabric version.',
  'Версии загрузчика обновлены.': 'Loader versions updated.',
  'Получить версии Fabric': 'Fetch Fabric versions',
  'stage:loader': 'Installing Fabric',

  'Логи игры': 'Game logs',
  'Прокручивать к новым строкам': 'Follow new lines',
  'Вывод Minecraft': 'Minecraft output',
  'Логов пока нет. Они появятся после запуска Minecraft.': 'No logs yet. They will appear after Minecraft starts.',
  'Обновление каждую секунду · последние 256 КБ · stdout и stderr': 'Refreshes every second · latest 256 KB · stdout and stderr',
  'LOG_READ_FAILED': 'Could not read the game log.',
  'LOG_INVALID_INSTANCE': 'Invalid instance identifier.',

  'Вход через официальную страницу Microsoft.': 'Sign in on the official Microsoft website.',
  'Для Microsoft-входа используется приложение Midnight. Доступ к Minecraft API ожидает одобрения.': 'Microsoft sign-in uses the Midnight application. Minecraft API access is awaiting approval.',

  'Оффлайн-профиль': 'Offline profile',
  'Ник': 'Username',
  'Ник: 1–16 латинских букв, цифр или _.': 'Username: 1–16 ASCII letters, digits, or _.',
  'Оффлайн-профиль выбран.': 'Offline profile selected.',
  'Выбрать оффлайн-профиль': 'Use offline profile',
  'Использовать Microsoft': 'Use Microsoft',
  'Локальная игра без Microsoft. Серверы с проверкой лицензии недоступны. Первая установка требует интернета.': 'Local play without Microsoft. Servers requiring authenticated accounts are unavailable. Initial installation requires internet.',
  'OFFLINE_INVALID_NAME': 'Invalid offline username.',

  "Главная": "Home",
  "Инстансы": "Instances",
  "Аккаунты": "Accounts",
  "Настройки": "Settings",
  "Язык интерфейса": "Interface language",
  "Перевод применяется сразу и сохраняется на этом устройстве.": "Changes apply immediately and are saved on this device.",
  "Тема оформления": "Appearance",
  "Выберите палитру для всех экранов лаунчера.": "Choose a palette for every screen of the launcher.",
  "Розовый сумрак": "Rose dusk",
  "Классический фиолетовый": "Classic purple",
  "Графит": "Graphite",
  "Градиент фона": "Background gradient",
  "Плавное переливание оттенков выбранной темы.": "Blend the shades of your selected theme.",
  "Midnight Launcher — главная": "Midnight Launcher — home",
  "О проекте": "About",
  "ТВОИ МИРЫ. ТВОЁ ВРЕМЯ.": "YOUR WORLDS. YOUR TIME.",
  "Игра запущена.": "Game launched.",
  "Подождите…": "Please wait…",
  "Редактировать выбор инстанса": "Manage instances",
  "ТЕКУЩИЙ ИНСТАНС": "CURRENT INSTANCE",
  "Найти инстанс": "Find an instance",
  "Каркас интерфейса · запуск игры в разработке": "UI scaffold · game launching is in development",
  "Сделано для ночных приключений": "Made for late-night adventures",
  "ТВОЯ КОЛЛЕКЦИЯ": "YOUR COLLECTION",
  "Отдельное место для каждой истории.": "A separate place for every story.",
  "Создать": "Create",
  "Название": "Name",
  "Например, Тихая долина": "For example, Quiet Valley",
  "Версия": "Version",
  "Создать инстанс": "Create instance",
  "Поиск инстансов": "Search instances",
  "Инстансы не найдены.": "No instances found.",
  "Выбран": "Selected",
  "Активный": "Active",
  "Выбрать": "Select",
  "Удалить": "Delete",
  "Файлы Minecraft": "Minecraft files",
  "Для «{name}» можно скачать client.jar и метаданные.": "Download client.jar and metadata for “{name}”.",
  "Это часть установщика: библиотеки, assets и Java пока не загружаются.": "Partial installer: libraries, assets and Java are not downloaded yet.",
  "Обновить версии": "Refresh versions",
  "Скачать client.jar": "Download client.jar",
  "Получено версий: {count}. Они доступны в поле версии при создании инстанса.": "Loaded {count} versions. They are available in the version field when creating an instance.",
  "ТВОЯ ИДЕНТИЧНОСТЬ": "YOUR IDENTITY",
  "Один профиль. Бесконечно много миров.": "One profile. Endless worlds.",
  "НЕТ ПОДКЛЮЧЁННОГО АККАУНТА": "NO ACCOUNT CONNECTED",
  "Твоё место в мире.": "Your place in the world.",
  "Вход через Microsoft появится здесь.": "Microsoft sign-in will be available here.",
  "Пароли и токены сейчас не запрашиваются.": "No passwords or tokens are requested right now.",
  "Войти через Microsoft": "Sign in with Microsoft",
  "Авторизация ещё не реализована": "Authentication is not implemented yet",
  "Будущий вход — через официальный сайт Microsoft.": "Future sign-in will use the official Microsoft website.",
  "Лаунчер не должен получать ваш пароль.": "The launcher should never receive your password.",
  "Подготовка интеграции": "Integration setup",
  "Собственный Client ID задаётся в настройках. Даже с ним вход пока недоступен.": "Set your own Client ID in Settings. Sign-in is not available yet, even with an ID.",
  "Проверить конфигурацию": "Check configuration",
  "ВСЁ ПОД КОНТРОЛЕМ": "MAKE IT YOURS",
  "Твой лаунчер. Твои правила.": "Your launcher. Your rules.",
  "Автосохранение": "Saved automatically",
  "Оперативная память": "Memory allocation",
  "Будущий лимит JVM. Пока сохраняется как настройка.": "Future JVM memory limit. Currently saved as a preference only.",
  "Путь к Java": "Java executable path",
  "Например, /path/to/java": "For example, /path/to/java",
  "Сохраняется локально. Поиск, проверка Java и запуск JVM ещё не реализованы.": "Saved locally. Java discovery, validation and JVM launching are not implemented yet.",
  "Client ID вашего собственного приложения": "Client ID of your own application",
  "Публичный идентификатор, не секрет. Никакой чужой Client ID не встроен. OAuth пока заглушка.": "A public identifier, not a secret. No borrowed Client ID is included. OAuth is a placeholder.",
  "Меньше движения": "Reduce motion",
  "Отключить декоративную анимацию интерфейса.": "Turn off decorative interface animations.",
  "Не удалось сохранить настройки. Данные сохранятся только до закрытия окна.": "Could not save settings. Changes will only last until the window closes.",
  "Закрыть уведомление": "Dismiss notification",
  "Основная навигация": "Main navigation",
  "Закрыть": "Close",
  "Каркас десктопного Minecraft Java лаунчера.": "A desktop Minecraft Java launcher scaffold.",
  "Инстансы и настройки сохраняются локально.": "Instances and settings are saved locally.",
  "Запуск Minecraft и Microsoft OAuth ещё не реализованы.": "Minecraft launching and Microsoft OAuth are not implemented yet.",
  "Загрузчик получает только client.jar и метаданные.": "The downloader only retrieves client.jar and metadata.",
  "Понятно": "Got it",
  "Достигнут лимит: 100 инстансов.": "The limit of 100 instances has been reached.",
  "Инстанс создан. Файлы игры ещё не установлены.": "Instance created. Game files are not installed yet.",
  "Название должно содержать от 1 до 40 символов.": "The name must contain between 1 and 40 characters.",
  "Укажите корректный ID версии, например 1.21.1.": "Enter a valid version ID, such as 1.21.1.",
  "Эта функция доступна в десктопном приложении. Откройте его командой npm run tauri dev.": "This feature requires the desktop app. Start it with npm run tauri dev.",
  "Client.jar и метаданные скачаны. Полная установка и запуск игры ещё не реализованы.": "Client.jar and metadata downloaded. Full installation and game launching are not implemented yet.",
  "Формат Client ID корректен. Регистрация не проверена. OAuth ещё не реализован.": "Client ID format is valid. Registration has not been verified. OAuth is not implemented yet.",
  "Укажите Client ID собственного приложения Microsoft в формате UUID. OAuth ещё не реализован.": "Enter the UUID Client ID of your own Microsoft application. OAuth is not implemented yet.",
  "Загрузка уже выполняется. Повторите позже.": "A download is already running. Try again later.",
  "Версия не найдена в официальном manifest Mojang.": "Version not found in the official Mojang manifest.",
  "У этой версии нет client.jar в metadata. Старые версии пока не поддерживаются.": "This version has no client.jar in its metadata. These older versions are not supported yet.",
  "Запуск Minecraft ещё не реализован: нужны полный установщик, проверенная Java, профиль Minecraft и авторизация. Кнопка Launch пока демонстрирует интерфейс.": "Minecraft launching is not implemented yet. A complete installer, validated Java, a Minecraft profile and authentication are required. Launch currently demonstrates the interface.",
  "Игра запущена": "Game is running",
  "Игра завершилась. Код: {code}": "Game exited. Code: {code}",
  "Аккаунт Minecraft подключён.": "Minecraft account connected.",
  "Vanilla · Apple Silicon · {name}": "Vanilla · Apple Silicon · {name}",
  "Войдите в Microsoft": "Sign in with Microsoft",
  "Будут установлены клиент, библиотеки, ресурсы и подходящая ARM64 Java.": "Installs the client, libraries, assets and matching ARM64 Java.",
  "Установить Minecraft для «{name}».": "Install Minecraft for “{name}”.",
  "Minecraft и Java установлены: {path}": "Minecraft and Java installed: {path}",
  "Установить Minecraft": "Install Minecraft",
  "ПОДТВЕРЖДЕНИЕ MICROSOFT": "MICROSOFT VERIFICATION",
  "Введите код на сайте Microsoft": "Enter this code on Microsoft",
  "Откройте официальный сайт, введите код и завершите вход. Пароль вводится только на Microsoft.": "Open the official website, enter the code and finish signing in. Enter your password only on Microsoft.",
  "Код действует {minutes} мин. Ожидаем подтверждение…": "Code expires in {minutes} min. Waiting for confirmation…",
  "Microsoft открыт в системном браузере.": "Microsoft opened in your system browser.",
  "Открыть Microsoft": "Open Microsoft",
  "Отменить вход": "Cancel sign-in",
  "Аккаунт сохранён. Перед запуском сессия обновляется и лицензия проверяется.": "Account saved. Your session and license are verified before launch.",
  "Вы вышли из аккаунта.": "Signed out.",
  "Выйти из аккаунта": "Sign out",
  "Вход через Microsoft. Нужна лицензия Minecraft Java Edition.": "Sign in with Microsoft. A Minecraft Java Edition license is required.",
  "Используется Client ID из настроек.": "Uses the Client ID from Settings.",
  "Refresh token хранится в Keychain. Пароль и токены не передаются в интерфейс.": "Refresh tokens are stored in Keychain. Passwords and tokens are never sent to this interface.",
  "Настройка Microsoft": "Microsoft setup",
  "Для первого входа зарегистрируйте собственное приложение Microsoft и укажите Client ID. Может потребоваться одобрение Minecraft API.": "Register your own Microsoft application and set its Client ID before signing in. Minecraft API approval may be required.",
  "Максимальная память для запуска Minecraft.": "Maximum memory allocated to Minecraft.",
  "Автоматически (ARM64 Java по версии Minecraft)": "Automatic (ARM64 Java matching Minecraft)",
  "Оставьте пустым для автоматической установки Java. Собственный путь проверяется по версии и архитектуре.": "Leave blank to install Java automatically. Custom paths are checked for the correct version and architecture.",
  "Client ID собственного публичного приложения с поддержкой личных аккаунтов и Device Code Flow. Client secret не нужен.": "Client ID of your own public app supporting personal accounts and Device Code Flow. No client secret is needed.",
  "Для запуска нужны подходящая Java, лицензия Minecraft и настроенный Microsoft Client ID.": "Launching requires matching Java, a Minecraft license and your configured Microsoft Client ID.",
  "Установщик загружает Minecraft, библиотеки, ресурсы и ARM64 Java.": "Installs Minecraft, libraries, assets and ARM64 Java.",
  "Десктопный Minecraft Java лаунчер для Apple Silicon.": "A desktop Minecraft Java launcher for Apple Silicon.",
  "Игра остановлена.": "Game stopped.",
  "Отмена запрошена.": "Cancellation requested.",
  "Остановить игру": "Stop game",
  "Отменить": "Cancel",
  "stage:preparing": "Preparing…",
  "stage:metadata": "Loading version metadata…",
  "stage:client": "Installing Minecraft client…",
  "stage:libraries": "Installing libraries…",
  "stage:assets": "Installing game assets…",
  "stage:installed": "Game files verified",
  "stage:java": "Preparing ARM64 Java…",
  "stage:ready": "Ready to play",
  "stage:running": "Minecraft is running"
};
const russianStages: Record<string,string> = {
  "stage:preparing": "Подготовка…",
  "stage:metadata": "Получение версии…",
  "stage:client": "Установка клиента…",
  "stage:libraries": "Установка библиотек…",
  "stage:assets": "Установка ресурсов…",
  "stage:installed": "Файлы игры проверены",
  "stage:java": "Подготовка ARM64 Java…",
  "stage:ready": "Готово к запуску",
  "stage:running": "Minecraft запущен"
};
const serviceErrors: Record<string, [string,string]> = {
  "AUTH_CLIENT_ID_REQUIRED": [
    "Укажите собственный Microsoft Client ID в настройках. Инструкция есть в MICROSOFT_SETUP.md.",
    "Set your own Microsoft Client ID in Settings. See MICROSOFT_SETUP.md."
  ],
  "AUTH_CONFIGURED": [
    "Формат Client ID корректен. Регистрация и одобрение приложения не проверены.",
    "Client ID format is valid. Registration and approval have not been verified."
  ],
  "AUTH_LOGIN_REQUIRED": [
    "Сначала войдите через Microsoft в разделе «Аккаунты».",
    "Sign in with Microsoft under Accounts first."
  ],
  "AUTH_MINECRAFT_APP_APPROVAL_REQUIRED": [
    "Minecraft отклонил приложение: проверьте одобрение Client ID для Minecraft API.",
    "Minecraft rejected this app. Verify that its Client ID is approved for Minecraft API access."
  ],
  "AUTH_MINECRAFT_LICENSE_REQUIRED": [
    "У аккаунта нет действующей лицензии Minecraft Java Edition.",
    "This account has no valid Minecraft Java Edition license."
  ],
  "AUTH_MINECRAFT_PROFILE_REQUIRED": [
    "Сначала создайте профиль Minecraft Java на minecraft.net.",
    "Create your Minecraft Java profile at minecraft.net first."
  ],
  "AUTH_XBOX_PROFILE_REQUIRED": [
    "У аккаунта нет профиля Xbox. Создайте его на xbox.com.",
    "This account needs an Xbox profile. Create it at xbox.com."
  ],
  "AUTH_XBOX_FAMILY_REQUIRED": [
    "Microsoft требует настройки семейной группы Xbox для этого аккаунта.",
    "Microsoft requires Xbox family settings for this account."
  ],
  "AUTH_MICROSOFT_expired_token": [
    "Код входа истёк. Начните вход ещё раз.",
    "The sign-in code expired. Start again."
  ],
  "AUTH_MICROSOFT_authorization_declined": [
    "Вход отклонён в Microsoft.",
    "Sign-in was declined on Microsoft."
  ],
  "AUTH_MICROSOFT_invalid_grant": [
    "Сессия Microsoft истекла. Выйдите из аккаунта и войдите снова.",
    "Your Microsoft session expired. Sign out and sign in again."
  ],
  "AUTH_MICROSOFT_invalid_client": [
    "Microsoft не распознал Client ID. Проверьте настройки приложения.",
    "Microsoft did not recognize this Client ID. Check your app registration."
  ],
  "AUTH_MICROSOFT_unauthorized_client": [
    "Microsoft не разрешает Device Code Flow для этого приложения. Проверьте Allow public client flows.",
    "Device Code Flow is not enabled for this app. Check Allow public client flows."
  ],
  "AUTH_MICROSOFT_invalid_scope": [
    "Microsoft отклонил разрешения XboxLive.signin / offline_access. Проверьте тип приложения.",
    "Microsoft rejected XboxLive.signin / offline_access. Check the app account type."
  ],
  "KEYCHAIN_WRITE_FAILED": [
    "Не удалось сохранить refresh token в Keychain. Вход не завершён.",
    "Could not save the refresh token in Keychain. Sign-in is incomplete."
  ],
  "KEYCHAIN_READ_FAILED": [
    "Нет доступа к записи Midnight в Keychain. Проверьте разрешение macOS.",
    "Cannot read the Midnight Keychain entry. Check macOS permission."
  ],
  "JAVA_ARM64_REQUIRED": [
    "Для этого Mac нужна ARM64 Java. Очистите путь Java для автоматической установки.",
    "This Mac requires ARM64 Java. Clear the Java path to install it automatically."
  ],
  "JAVA_EXECUTABLE_NOT_FOUND": [
    "Java не найдена. Очистите поле пути для автоматической установки.",
    "Java was not found. Clear the custom path to install it automatically."
  ],
  "INSTALL_ARM64_VERSION_REQUIRED": [
    "Эта версия не содержит ARM64-библиотеки. Используйте современную Vanilla, например 1.21.1.",
    "This version has no ARM64 natives. Use a modern vanilla version such as 1.21.1."
  ],
  "OPERATION_ALREADY_RUNNING": [
    "Уже выполняется установка или запуск.",
    "An installation or launch is already running."
  ],
  "OPERATION_CANCELLED": [
    "Операция отменена. Уже проверенные файлы сохранены.",
    "Operation cancelled. Verified files have been kept."
  ],
  "GAME_ALREADY_RUNNING": [
    "Minecraft уже запущен.",
    "Minecraft is already running."
  ]
};
export function translate(language: Language, text: string): string {
  if (serviceErrors[text]) return serviceErrors[text][language === 'en' ? 1 : 0];
  if (text.startsWith('JAVA_MAJOR_REQUIRED_')) return language === 'en' ? `Java ${text.slice(20)} is required. Clear the custom path for automatic installation.` : `Требуется Java ${text.slice(20)}. Очистите путь для автоматической установки.`;
  if (/^(AUTH_|KEYCHAIN_|INSTALL_|CACHE_|JAVA_|LAUNCH_|GAME_)/.test(text)) {
    const label = text.startsWith('AUTH_') ? (language === 'en' ? 'Authentication failed' : 'Ошибка авторизации') : (language === 'en' ? 'Operation failed' : 'Ошибка операции');
    return `${label}: ${text}`;
  }
  return language === 'en' ? (english[text] ?? text) : (russianStages[text] ?? text);
}
