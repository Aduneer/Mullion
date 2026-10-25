; Mullion's installer: Mullion.exe (the suite's tray icon, sign-in entry and settings) and the apps
; you tick, all in one folder, %LOCALAPPDATA%\Mullion, with one Start menu entry and one entry in
; Installed apps. Mullion starts the apps from there with --mullion (core/src/suite.rs), so none of
; them has a tray icon, sign-in entry or uninstaller of its own. Each app's own installer is still
; in the release, for anyone who wants just that one.
; The release workflow builds it after the apps, with:
;   makensis /DVERSION=0.3.0 /DBIN=<folder with the release exes> /DOUTFILE=<exe> mullion.nsi

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "Sections.nsh"
!include "FileFunc.nsh"

!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall"
!define RUN "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
; The WebView2 runtime's id with Edge's updater, which records the version it installed.
!define WEBVIEW2 "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Name "Mullion"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Mullion"
; Everything installs per user, so it needs no admin rights.
RequestExecutionLevel user
BrandingText "Mullion ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Mullion"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "Mullion setup"
VIAddVersionKey "CompanyName" "Aduneer"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 Aduneer. MIT License."

!define MUI_ICON "..\apps\mullion\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\apps\mullion\src-tauri\icons\icon.ico"
!define MUI_COMPONENTSPAGE_SMALLDESC
!define MUI_WELCOMEPAGE_TEXT "Mullion reworks the Windows desktop, one part at a time. Pick the apps you want on the next page; Mullion Settings turns any of them on or off later.$\r$\n$\r$\nEverything installs for you only, without admin rights."
!define MUI_FINISHPAGE_RUN "$INSTDIR\mullion.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Mullion"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Quit a running Mullion and the apps it runs, so their files can be replaced or removed.
!macro QuitMullion
  ${If} ${FileExists} "$INSTDIR\mullion.exe"
    ExecWait '"$INSTDIR\mullion.exe" --quit'
    Sleep 2000
  ${EndIf}
!macroend

Section "Mullion" SecMullion
  SectionIn RO
  !insertmacro QuitMullion
  SetOutPath "$INSTDIR"
  File "${BIN}\mullion.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Mullion.lnk" "$INSTDIR\mullion.exe"
  WriteRegStr HKCU "${UNINST}\Mullion" "DisplayName" "Mullion"
  WriteRegStr HKCU "${UNINST}\Mullion" "DisplayIcon" '"$INSTDIR\mullion.exe"'
  WriteRegStr HKCU "${UNINST}\Mullion" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}\Mullion" "Publisher" "Aduneer"
  WriteRegStr HKCU "${UNINST}\Mullion" "URLInfoAbout" "https://github.com/Aduneer/Mullion"
  WriteRegStr HKCU "${UNINST}\Mullion" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${UNINST}\Mullion" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST}\Mullion" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST}\Mullion" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}\Mullion" "NoRepair" 1
SectionEnd

; One section per app: its exe beside Mullion's. If the app is also installed on its own, offer to
; remove that copy (its settings stay, in %APPDATA%\<App>, and this one uses them).
!macro App NAME EXE DESCRIPTION
  Section "${NAME}" Sec${NAME}
    SetOutPath "$INSTDIR"
    File "${BIN}\${EXE}.exe"
    ; Mullion starts it now; a sign-in entry of its own would start it twice.
    DeleteRegValue HKCU "${RUN}" "${NAME}"
    DeleteRegValue HKCU "${APPROVED}" "${NAME}"
    ReadRegStr $1 HKCU "${UNINST}\${NAME}" "UninstallString"
    ReadRegStr $2 HKCU "${UNINST}\${NAME}" "InstallLocation"
    ${If} $1 != ""
    ${AndIf} $2 != ""
      MessageBox MB_YESNO|MB_ICONQUESTION "${NAME} is also installed on its own. Mullion has its own copy of it now. Remove the separate one?$\r$\n$\r$\nYour ${NAME} settings stay either way." /SD IDYES IDNO skip_${EXE}
      StrCpy $2 $2 "" 1
      StrCpy $2 $2 -1
      DetailPrint "Removing the separate ${NAME}..."
      ExecWait '"$2\${EXE}.exe" --quit'
      Sleep 1500
      ; _?= runs the uninstaller in place, so this waits for it.
      ExecWait '$1 /S _?=$2'
      Delete "$2\uninstall.exe"
      RMDir "$2"
      skip_${EXE}:
    ${EndIf}
  SectionEnd
  LangString DESC_${NAME} ${LANG_ENGLISH} "${DESCRIPTION}"
!macroend

!insertmacro App "Oriel" "oriel" "A command centre for your second screen: your PC's vitals, calendar, weather, what's playing and more."
!insertmacro App "Lintel" "lintel" "A menu bar along the top of the screen: the app in front, your open apps, what's playing and the clock."
!insertmacro App "Fanlight" "fanlight" "Search and launch on Alt+Space: apps, files, windows, sums and the clipboard."

Section "Start Mullion when I sign in" SecSignIn
  WriteRegStr HKCU "${RUN}" "Mullion" '"$INSTDIR\mullion.exe" --sign-in'
SectionEnd

; Last: remove the apps left unticked (when changing an install), and note the size.
Section "-Tidy"
  ${IfNot} ${SectionIsSelected} ${SecOriel}
    Delete "$INSTDIR\oriel.exe"
  ${EndIf}
  ${IfNot} ${SectionIsSelected} ${SecLintel}
    Delete "$INSTDIR\lintel.exe"
  ${EndIf}
  ${IfNot} ${SectionIsSelected} ${SecFanlight}
    Delete "$INSTDIR\fanlight.exe"
  ${EndIf}
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST}\Mullion" "EstimatedSize" "$0"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMullion} "Mullion itself: one icon in the notification area, one sign-in entry and one settings window for the apps you pick."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecOriel} $(DESC_Oriel)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecLintel} $(DESC_Lintel)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecFanlight} $(DESC_Fanlight)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecSignIn} "Start Mullion, and the apps you picked, each time you sign in to Windows."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInit
  ; Changing an install: start from what's there.
  ReadRegStr $0 HKCU "${UNINST}\Mullion" "InstallLocation"
  ${If} $0 != ""
    StrCpy $INSTDIR $0 "" 1
    StrCpy $INSTDIR $INSTDIR -1
    ${IfNot} ${FileExists} "$INSTDIR\oriel.exe"
      !insertmacro UnselectSection ${SecOriel}
    ${EndIf}
    ${IfNot} ${FileExists} "$INSTDIR\lintel.exe"
      !insertmacro UnselectSection ${SecLintel}
    ${EndIf}
    ${IfNot} ${FileExists} "$INSTDIR\fanlight.exe"
      !insertmacro UnselectSection ${SecFanlight}
    ${EndIf}
    ReadRegStr $0 HKCU "${RUN}" "Mullion"
    ${If} $0 == ""
      !insertmacro UnselectSection ${SecSignIn}
    ${EndIf}
  ${EndIf}
  ; The apps' pages are drawn by WebView2, which Windows 11 has and Edge brings to Windows 10.
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WEBVIEW2}" "pv"
  ${EndIf}
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    MessageBox MB_YESNO|MB_ICONEXCLAMATION "Mullion needs Microsoft's WebView2 runtime, which this PC doesn't have. Open its download page?" /SD IDNO IDNO +2
    ExecShell "open" "https://developer.microsoft.com/microsoft-edge/webview2/"
    Abort
  ${EndIf}
FunctionEnd

; --- uninstall ------------------------------------------------------------------------------

; An exe that has only just quit can stay locked for a moment, so try again for up to 10 s.
!macro DeleteWhenFree FILE
  StrCpy $R0 0
  ${Do}
    ${IfNot} ${FileExists} "${FILE}"
    ${OrIf} $R0 >= 20
      ${Break}
    ${EndIf}
    Delete "${FILE}"
    ${If} ${FileExists} "${FILE}"
      Sleep 500
    ${EndIf}
    IntOp $R0 $R0 + 1
  ${Loop}
!macroend

Section "un.Mullion and its apps" SecUnMullion
  SectionIn RO
  !insertmacro QuitMullion
  !insertmacro DeleteWhenFree "$INSTDIR\mullion.exe"
  !insertmacro DeleteWhenFree "$INSTDIR\oriel.exe"
  !insertmacro DeleteWhenFree "$INSTDIR\lintel.exe"
  !insertmacro DeleteWhenFree "$INSTDIR\fanlight.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Mullion.lnk"
  DeleteRegValue HKCU "${RUN}" "Mullion"
  DeleteRegValue HKCU "${APPROVED}" "Mullion"
  DeleteRegKey HKCU "${UNINST}\Mullion"
SectionEnd

; An app's settings and its WebView's data, unless it's still installed on its own.
!macro UnData NAME IDENTIFIER
  ReadRegStr $0 HKCU "${UNINST}\${NAME}" "UninstallString"
  ${If} $0 == ""
    RMDir /r "$APPDATA\${NAME}"
    RMDir /r "$LOCALAPPDATA\${IDENTIFIER}"
  ${EndIf}
!macroend

Section /o "un.Delete the application data" SecUnData
  RMDir /r "$APPDATA\Mullion"
  RMDir /r "$LOCALAPPDATA\com.aduneer.mullion"
  !insertmacro UnData "Oriel" "dev.oriel.dashboard"
  !insertmacro UnData "Lintel" "com.aduneer.lintel"
  !insertmacro UnData "Fanlight" "com.aduneer.fanlight"
SectionEnd

!insertmacro MUI_UNFUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnMullion} "Mullion and every app installed with it."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnData} "Also the apps' settings, history and saved data, except for an app that's still installed on its own."
!insertmacro MUI_UNFUNCTION_DESCRIPTION_END

Function un.onInit
  ReadRegStr $0 HKCU "${UNINST}\Mullion" "InstallLocation"
  ${If} $0 != ""
    StrCpy $INSTDIR $0 "" 1
    StrCpy $INSTDIR $INSTDIR -1
  ${EndIf}
FunctionEnd
