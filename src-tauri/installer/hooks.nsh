; This file is included before Tauri expands its stock Modern UI pages.
; Keep the upstream installation and uninstallation logic unchanged.
!define MIDNIGHT_ASSETS "${__FILEDIR__}\..\target\installer"
!include "${MIDNIGHT_ASSETS}\colors.nsh"
!define MUI_BGCOLOR "${MIDNIGHT_BACKGROUND}"
!define MUI_TEXTCOLOR "${MIDNIGHT_INK}"
!define MUI_INSTFILESPAGE_COLORS "${MIDNIGHT_INK} ${MIDNIGHT_DEEP}"
!define MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH FitControl
!define MUI_HEADERIMAGE_BITMAP_STRETCH AspectFitHeight
!define MUI_HEADERIMAGE_RIGHT
!define MUI_WELCOMEPAGE_TITLE "$(MidnightWelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(MidnightWelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(MidnightFinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(MidnightFinishText)"
!define MUI_DIRECTORYPAGE_TEXT_TOP "$(MidnightDirectoryText)"
!define MUI_CUSTOMFUNCTION_GUIINIT MidnightThemeInit
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.MidnightThemeInit

; Extend MUI's language loader with the matching generated Midnight strings.
; LoadLanguageFile must run first: declaring an unused language before MUI
; creates an implicit table with untranslated default Windows controls.
!macroundef MUI_LANGUAGE
!macro MUI_LANGUAGE NLFID
  !insertmacro MUI_LANGUAGEEX "${NSISDIR}\Contrib\Language files" "${NLFID}"
  !if "${NLFID}" == "Russian"
    !include "${MIDNIGHT_ASSETS}\strings-Russian.nsh"
  !else if "${NLFID}" == "English"
    !include "${MIDNIGHT_ASSETS}\strings-English.nsh"
  !else
    !error "Midnight installer supports Russian and English. Add installer translations before adding a language."
  !endif
!macroend

!macro MidnightThemeInit PREFIX
Function ${PREFIX}MidnightThemeInit
  InitPluginsDir
  File /oname=$PLUGINSDIR\midnight-theme.dll "${MIDNIGHT_ASSETS}\midnight-theme.dll"
  System::Call '"$PLUGINSDIR\midnight-theme.dll"::Apply(p $HWNDPARENT)'
FunctionEnd
!macroend
!insertmacro MidnightThemeInit ""
!insertmacro MidnightThemeInit "un."
