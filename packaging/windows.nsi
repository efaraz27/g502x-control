Unicode True
!include "MUI2.nsh"
Name "G502 X Control"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\G502 X Control"
InstallDirRegKey HKCU "Software\G502 X Control" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "${SOURCE}\packaging\g502x-control.ico"
UninstallIcon "${SOURCE}\packaging\g502x-control.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\g502x-control.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open G502 X Control"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${SOURCE}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\G502 X Control"
  CreateShortcut "$SMPROGRAMS\G502 X Control\G502 X Control.lnk" "$INSTDIR\g502x-control.exe"
  WriteRegStr HKCU "Software\G502 X Control" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "DisplayName" "G502 X Control"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "Publisher" "G502 X Control contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "DisplayIcon" "$INSTDIR\g502x-control.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control" "NoRepair" 1
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\g502x-control.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\licenses"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\G502 X Control\G502 X Control.lnk"
  RMDir "$SMPROGRAMS\G502 X Control"
  DeleteRegKey HKCU "Software\G502 X Control"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\G502 X Control"
  # User backups under LOCALAPPDATA\g502x-control are deliberately retained.
SectionEnd
