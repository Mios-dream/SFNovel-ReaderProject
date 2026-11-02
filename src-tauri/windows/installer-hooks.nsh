; 卸载时清理应用自行写入的开机启动项。
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SF Novel Flow"
!macroend
