!macro NSIS_RUN_BACKEND_CLEANUP
  ; Ensure packaged backend processes do not keep install files locked.
  StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$0" +2 0
    StrCpy $0 "powershell.exe"
  StrCpy $1 "$INSTDIR\kill-backend-processes.ps1"
  ; During updater-driven installs Tauri can stage the incoming bundle under `_up_\resources`
  ; before files are copied into the final install root, so keep that path as a fallback.
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
      StrCpy $2 "$0\.astrbot"
      StrCpy $3 "警告：将永久删除 AstrBot 的本地数据目录：$2$\r$\n$\r$\n包括配置、日志、数据库、插件数据和缓存，删除后不可恢复。$\r$\n$\r$\n确定继续删除吗？$\r$\n$\r$\nWarning: This will permanently remove AstrBot local data at:$\r$\n$2$\r$\n$\r$\nThis includes config/logs/databases/plugin data/cache and cannot be recovered.$\r$\n$\r$\nContinue?"
      MessageBox MB_ICONEXCLAMATION|MB_YESNO|MB_DEFBUTTON2 "$3" IDYES astrbot_delete_app_data_confirmed
      StrCpy $DeleteAppDataCheckboxState 0
      DetailPrint "Skip app data cleanup: user canceled delete confirmation."
      Goto astrbot_delete_app_data_done
      astrbot_delete_app_data_confirmed:
      RmDir /r "$2"
      astrbot_delete_app_data_done:
    ${Else}
      DetailPrint "Skip app data cleanup: USERPROFILE is empty."
    ${EndIf}
  ${EndIf}
!macroend
