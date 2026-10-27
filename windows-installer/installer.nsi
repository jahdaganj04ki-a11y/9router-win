; 9Router NSIS Installer
; Build with makensis on a Windows host.

!define APP_NAME "9router"
!define APP_VERSION "0.5.111"
!define APP_PUBLISHER "decolua"
!define APP_EXE "9router.exe"
!define UNINSTALLER_NAME "uninstall.exe"
!define REG_KEY "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${APP_NAME}"
!define RUN_KEY "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
!define RUN_VALUE "${APP_NAME}"

!include "MUI2.nsh"
!include "x64.nsh"
!include "WinVer.nsh"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "9router_setup_${APP_VERSION}.exe"
InstallDir "$PROGRAMFILES64\\${APP_NAME}"
RequestExecutionLevel admin
ShowInstDetails show
ShowUnInstDetails show

!define MUI_ABORTWARNING
!define MUI_ICON "src-tauri/icons/icon.ico"
!define MUI_UNICON "src-tauri/icons/icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "English"

; Main application section
Section "9router" SecMain
  SetOutPath "$INSTDIR"

  ; Tauri binary (built for x86_64-pc-windows-msvc)
  File "src-tauri/target/x86_64-pc-windows-msvc/release/9router.exe"

  ; Node.js binary
  SetOutPath "$INSTDIR\\resources\\node"
  File /r "src-tauri/resources/node/*"

  ; Standalone app
  SetOutPath "$INSTDIR\\resources\\app"
  File /r "src-tauri/resources/app/*"

  ; Uninstaller
  WriteUninstaller "$INSTDIR\\${UNINSTALLER_NAME}"

  ; Registry: Uninstall info
  WriteRegStr HKLM "${REG_KEY}" "DisplayName" "${APP_NAME} ${APP_VERSION}"
  WriteRegStr HKLM "${REG_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "${REG_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "${REG_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${REG_KEY}" "UninstallString" '"$INSTDIR\\${UNINSTALLER_NAME}"'
  WriteRegStr HKLM "${REG_KEY}" "DisplayIcon" '"$INSTDIR\\${APP_EXE}"'

  ; Autostart is NOT set by default (user toggles in-app)
SectionEnd

Section "Desktop Shortcut" SecDesktop
  CreateShortcut "$DESKTOP\\${APP_NAME}.lnk" "$INSTDIR\\${APP_EXE}"
SectionEnd

Section "Start Menu Shortcut" SecStartMenu
  CreateShortcut "$SMPROGRAMS\\${APP_NAME}.lnk" "$INSTDIR\\${APP_EXE}"
SectionEnd

Function .onInit
  ${If} ${FileExists} "$SYSDIR\\WebView2Loader.dll"
    DetailPrint "WebView2 Runtime ist installiert."
  ${Else}
    DetailPrint "WebView2 Runtime fehlt. Nach der Installation kann der Bootstrapper nachgeladen werden."
  ${EndIf}

  ${If} ${FileExists} "$SYSDIR\\msvcp140.dll"
    DetailPrint "VC++ Redistributable ist vorhanden."
  ${Else}
    DetailPrint "Hinweis: VC++ Redistributable könnte fehlen."
  ${EndIf}
FunctionEnd

Function un.onInit
  DeleteRegValue HKCU "${RUN_KEY}" "${RUN_VALUE}"
FunctionEnd

Function .onFinish
  Exec '"$INSTDIR\\${APP_EXE}"'
FunctionEnd
