; 9router NSIS-Installer (Windows)
; Bauen auf einem Windows-Host:  makensis installer.nsi
; Signieren (optional):          signtool sign /a /f cert.pfx /p <pw> /tr http://timestamp.digicert.com /td SHA256 9router_setup.exe

!include "Mui2.nsh"
!include "Mui.nsh"

!define APP_NAME "9router"
!define APP_VERSION "0.5.111"
!define INSTALLER_NAME "9router_setup.exe"
!define DEFAULT_DIR "C:\Program Files\9router"
!define RUN_KEY "HKCU\Software\Microsoft\Windows\CurrentVersion\Run"
!define RUN_VALUE "9router"

!installlang LANG_ENGLISH
!define MUI_ABORTWARNING
!define MUI_ICON "src-tauri\icons\icon.ico"
!define MUI_UNICON "src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

Name "${APP_NAME} ${APP_VERSION}"
OutFile "..\..\dist\${INSTALLER_NAME}"
InstallDir "${DEFAULT_DIR}"
RequestExecutionLevel admin
ShowInstDetails show

Section "9router-App" SecApp
  SetOutPath "$INSTDIR"
  File /r "src-tauri\resources\node\*.*"
  File /r "src-tauri\resources\app\*.*"
  File "9router.exe"
  CreateShortCut "$SMPROGRAMMS\${APP_NAME}.lnk" "$INSTDIR\${APP_NAME}.exe"
SectionEnd

Section "Desktop-Verknüpfung" SecDesktop
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_NAME}.exe"
SectionEnd

Section -Uninstall
  Delete "$INSTDIR\${APP_NAME}.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\resources"
  Delete "$SMPROGRAMMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  DeleteRegValue ${RUN_KEY} "${RUN_VALUE}"
  ; DATA_DIR (%APPDATA%\9router) bleibt bestehen — Nutzerdaten nicht löschen
  RMDir "$INSTDIR"
SectionEnd

Function .onInit
  !insertmacro MUI_RESERVEFILE_INSTALLOPTIONAL DeskIcon
FunctionEnd

Function un.onInit
  MessageBox MB_OKCANCEL "${APP_NAME} deinstallieren?" IDOK +2
  Abort
FunctionEnd

Function .onFinish
  ${If} ${FileExists} "$PROGRAMFILES64\Microsoft\EdgeWebView\Application\*\WebView2Loader.dll"
  ${Else}
    MessageBox MB_OK "Hinweis: Microsoft Edge WebView2 Runtime nicht gefunden. ${APP_NAME} benoetigt die WebView2-Runtime.\nBootstrapper: https://aka.ms/WebView2Bootstrapper"
  ${EndIf}
FunctionEnd
