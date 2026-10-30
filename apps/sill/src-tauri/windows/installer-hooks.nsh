; Extra steps for the installer Tauri generates (see bundle > windows > nsis in tauri.conf.json).

; Before files are replaced or removed: a running Sill puts the taskbar back as it quits.
!macro NSIS_HOOK_PREINSTALL
  ExecWait '"$INSTDIR\sill.exe" --quit'
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\sill.exe" --quit'
  ExecWait '"$INSTDIR\sill.exe" --restore-taskbar'
!macroend

; With "Delete the application data" ticked, also remove what Tauri doesn't know about: the
; settings in %APPDATA%\Sill and the "Start at sign-in" entry. Not during an update,
; and not when a newer installer runs this uninstaller first (the box isn't shown then), so
; upgrading keeps both.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Sill"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Sill"
    RMDir /r "$APPDATA\Sill"
  ${EndIf}
!macroend
