!macro NSIS_RUN_BACKEND_CLEANUP
  ; Ensure packaged backend processes do not keep install files locked.
  StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$0" +2 0
    StrCpy $0 "powershell.exe"
  StrCpy $1 "$INSTDIR\resources\kill-backend-processes.ps1"
  IfFileExists "$1" +2 0
    StrCpy $1 "$INSTDIR\_up_\resources\kill-backend-processes.ps1"
  IfFileExists "$1" 0 +3
    nsExec::ExecToLog '"$0" -NoProfile -ExecutionPolicy Bypass -File "$1" -InstallDir "$INSTDIR"'
    Goto +2
  DetailPrint "Skip backend process cleanup: script not found: $1"
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Stop old app/backend processes before overwriting files during upgrades.
  !insertmacro NSIS_RUN_BACKEND_CLEANUP
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro NSIS_RUN_BACKEND_CLEANUP
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Recreate shortcuts to avoid stale links when users migrate from older installers.
  StrCpy $0 "$INSTDIR\${MAINBINARYNAME}.exe"
  ${If} ${FileExists} "$0"
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$0"

    CreateDirectory "$SMPROGRAMS\${PRODUCTNAME}"
    Delete "$SMPROGRAMS\${PRODUCTNAME}\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}\${PRODUCTNAME}.lnk" "$0"
  ${Else}
    DetailPrint "Skip shortcut recreation: main binary not found at $0"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Keep behavior aligned with NSIS checkbox: only remove user data when user asked for it.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ExpandEnvStrings $0 "%USERPROFILE%"
    ${If} $0 != ""
      RmDir /r "$0\.astrbot"
    ${Else}
      DetailPrint "Skip app data cleanup: USERPROFILE is empty."
    ${EndIf}
  ${EndIf}
!macroend
