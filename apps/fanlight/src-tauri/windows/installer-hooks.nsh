; Extra steps for the installer Tauri generates (see bundle > windows > nsis in tauri.conf.json).

; With "Delete the application data" ticked, also remove what Tauri doesn't know about: the
; settings and history in %APPDATA%\Fanlight and the "Start at sign-in" entry. Not during an update,
; and not when a newer installer runs this uninstaller first (the box isn't shown then), so
; upgrading keeps both.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Fanlight"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Fanlight"
    RMDir /r "$APPDATA\Fanlight"
  ${EndIf}
!macroend
