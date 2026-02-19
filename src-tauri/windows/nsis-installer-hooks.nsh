!macro NSIS_HOOK_PREUNINSTALL
  ; Ensure packaged backend processes do not keep install files locked during uninstall.
  StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$0" +2 0
    StrCpy $0 "powershell.exe"
  StrCpy $1 "$$installRoot = [System.IO.Path]::GetFullPath(''$INSTDIR'').TrimEnd([char]92).ToLower()"
  StrCpy $2 "$$installRootWithSep = $$installRoot + [string][char]92"
  StrCpy $3 "Get-CimInstance Win32_Process -Filter \"Name=''python.exe'' OR Name=''pythonw.exe''\""
  StrCpy $4 "$3 | Where-Object { $$_.ExecutablePath } | ForEach-Object { $$exePath = [System.IO.Path]::GetFullPath($$_.ExecutablePath).ToLower(); if ($$exePath.StartsWith($$installRootWithSep) -or $$exePath -eq $$installRoot) { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue } }"
  nsExec::ExecToLog '"$0" -NoProfile -ExecutionPolicy Bypass -Command "$1; $2; $4"'
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
