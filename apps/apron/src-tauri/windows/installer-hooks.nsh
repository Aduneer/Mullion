; Extra steps for the installer Tauri generates (see bundle > windows > nsis in tauri.conf.json).

; Before files are replaced or removed: a running Apron puts the taskbar back as it quits.
!macro NSIS_HOOK_PREINSTALL
  ExecWait '"$INSTDIR\apron.exe" --quit'
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\apron.exe" --quit'
  ExecWait '"$INSTDIR\apron.exe" --restore-taskbar'
!macroend

; apron-guard.exe, which gives the taskbar back if Apron stops without doing it, goes beside
; apron.exe (built by beforeBundleCommand). The path is from the script Tauri writes, in
; target\release\nsis\x64.
!macro NSIS_HOOK_POSTINSTALL
  SetOutPath $INSTDIR
  File "..\..\apron-guard.exe"
!macroend

; Tauri's list of files doesn't have the guard, so its folder is left too.
; With "Delete the application data" ticked, also remove what Tauri doesn't know about: the
; settings in %APPDATA%\Apron and the "Start at sign-in" entry. Not during an update,
; and not when a newer installer runs this uninstaller first (the box isn't shown then), so
; upgrading keeps both.
!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\apron-guard.exe"
  RMDir "$INSTDIR"
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Apron"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Apron"
    RMDir /r "$APPDATA\Apron"
  ${EndIf}
!macroend
