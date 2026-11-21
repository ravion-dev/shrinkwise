!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  RMDir /r "$INSTDIR\temp"
  RMDir /r "$INSTDIR\logs"
  RMDir /r "$INSTDIR\tools"
  Delete "$INSTDIR\settings.json"
  Delete "$INSTDIR\history.jsonl"
  ${If} $DeleteAppDataCheckboxState == 1
    RMDir /r "$INSTDIR\Upscaled"
    RMDir /r "$LOCALAPPDATA\ShrinkWise"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
