; Extra steps for the FreeSheet Windows installer (NSIS, included by Tauri's bundler).

; FreeSheet 1.0.x was an Electron app with its own uninstaller. Remove it quietly before
; installing, so there is only one FreeSheet. Progress (in %APPDATA%\FreeSheet) is kept
; and the new version reads it from the same place.
!macro NSIS_HOOK_PREINSTALL
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\35d0bcbc-e5ae-5f8a-a0b0-0b1ba2d7a439" "UninstallString"
  StrCmp $R0 "" fs_old_done
  ; $R0 is: "C:\...\Uninstall FreeSheet.exe" /currentuser  ->  take the part in quotes.
  StrCpy $R1 $R0 "" 1
  StrCpy $R2 0
  fs_old_scan:
    StrCpy $R3 $R1 1 $R2
    StrCmp $R3 '"' fs_old_found
    StrCmp $R3 "" fs_old_done
    IntOp $R2 $R2 + 1
    Goto fs_old_scan
  fs_old_found:
  StrCpy $R1 $R1 $R2
  IfFileExists "$R1" 0 fs_old_done
  DetailPrint "Removing the old FreeSheet 1.0..."
  ; _?= makes the uninstaller run in place and wait; "\Uninstall FreeSheet.exe" is 24 characters.
  StrCpy $R2 $R1 -24
  ExecWait '"$R1" /currentuser /S _?=$R2'
  Delete "$R1"
  RMDir "$R2"
  fs_old_done:
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
!macroend
