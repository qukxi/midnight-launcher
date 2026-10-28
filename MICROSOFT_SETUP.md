# Настройка Microsoft для Midnight Launcher

В сборку добавлен Client ID владельца Midnight: `6b0fede0-1318-4335-908d-a668b7599747`. Это публичный идентификатор, не секрет. Пустое поле из старых настроек автоматически заполняется им; свой ранее сохранённый ID сохраняется. Включение public client flows и допуск Minecraft API ещё не подтверждены.

В Midnight нет встроенного чужого Client ID. Чтобы войти, владелец проекта должен зарегистрировать собственное публичное приложение. Один зарегистрированный Client ID затем используется пользователями этой сборки — каждому игроку не нужно регистрировать отдельное приложение.

## Регистрация

1. Откройте [Microsoft Entra — App registrations](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) и войдите своим аккаунтом владельца проекта. Если нет доступа к каталогу, потребуется доступный вам Entra tenant с правом регистрации приложений. Регистрация приложения и владение Minecraft — разные вещи.
2. Выберите **New registration**, название **Midnight Launcher**.
3. Выберите поддержку **Personal Microsoft accounts** (или вариант с организационными и личными аккаунтами). Midnight обращается к authority `consumers`; рабочие/учебные аккаунты сами по себе не подходят для Minecraft Java.
4. Для используемого Device Code Flow callback/redirect URI не используется. Секрет приложения не создавайте: десктопный лаунчер является публичным клиентом.
5. В **Authentication → Advanced settings → Allow public client flows** включите **Yes** и сохраните. В обновлённом интерфейсе Microsoft названия разделов могут отличаться.
6. Скопируйте **Application (client) ID** со страницы Overview. Это публичный UUID, не пароль.
7. Встроенный Client ID настраивается разработчиком в `src/model.ts`; поле скрыто в пользовательском интерфейсе.
8. Откройте **Accounts → Sign in with Microsoft**. Лаунчер запрашивает настоящий одноразовый код. Нажмите **Open Microsoft**, введите код и завершите вход на официальном сайте Microsoft.
9. Разрешения flow: `XboxLive.signin offline_access`. Midnight не запрашивает пароль, Microsoft Graph, доступ к почте или файлам.

Не присылайте в чат пароль, refresh token, client secret или коды MFA. В процессе реального входа может появиться штатное окно доступа macOS к записи Midnight в Keychain.

## Доступ к Minecraft API

Регистрация Client ID не означает, что Minecraft Services примет приложение. Проверьте актуальный процесс **Java Edition Game Service API Review or Application Process** через [официальный раздел поддержки Java Edition](https://help.minecraft.net/hc/en-us/sections/44460521913485). Если для приложения требуется заявка/одобрение Mojang, его нужно пройти отдельно.

При HTTP 403 на Minecraft Services Midnight сообщает об отказе и предлагает проверить одобрение приложения. Точную причину отказа нужно подтверждать по состоянию вашей регистрации и требованиям сервиса; не подставляйте Client ID Prism или официального Minecraft Launcher.

## Что проверяет лаунчер

Microsoft device code → Microsoft access token → Xbox user token → XSTS → Minecraft launcher token → `game_minecraft` entitlement → Minecraft profile. Успешным считается только завершение всей цепочки.

При истечении кода, отказе пользователя, ошибке настройки Client ID, отсутствии Xbox-профиля, семейных ограничениях или отсутствии лицензии вход завершается соответствующей ошибкой. Кнопка Cancel прекращает дальнейшие этапы; уже выполняющийся HTTP-запрос ограничен таймаутом. Подтверждённого успешного входа реальным аккаунтом у этой сборки пока нет, хотя Client ID владельца уже предоставлен.

Хранится один активный аккаунт. Для смены аккаунта выйдите из текущего, затем начните новый вход. Refresh token сохраняется в системном Keychain, обновляется при refresh и удаляется при Sign out. Access tokens остаются в памяти Rust и передаются только Microsoft/Xbox/Minecraft и JVM при запуске.

## Источники

- [Регистрация приложения Microsoft](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app)
- [Device Authorization Grant Microsoft](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code)
- [Включение public client flows](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-mobile-app-sign-in)
- [Требуемые аккаунты Minecraft](https://help.minecraft.net/hc/en-us/articles/19615552270221-Accounts-Required-to-Play-Minecraft)

Параметры цепочки Xbox/Minecraft сверены с актуальными первичными исходниками Prism (AuthFlow, XboxUserStep, XboxAuthorizationStep, LauncherLoginStep, EntitlementsStep и MinecraftProfileStep). Код реализации Midnight написан отдельно; чужие ID не используются. Minecraft API не имеет здесь гарантии неизменности: при изменении сервиса потребуется обновление реализации.
