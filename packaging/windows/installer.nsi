Unicode true
!include "FileFunc.nsh"

Name "PaperCraft"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\PaperCraft"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "@VERSION@.0"
VIAddVersionKey "ProductName" "PaperCraft"
VIAddVersionKey "FileDescription" "PaperCraft per-user installer"
VIAddVersionKey "FileVersion" "@VERSION@"
VIAddVersionKey "LegalCopyright" "PaperCraft contributors — MIT license"

Var Identity
Var VerifyToken
Var UninstallCommand

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

; Verification uses a unique HKCU entry and Start-menu link, never the user's
; normal PaperCraft registration. The token is also in its uninstall command.
!macro InitializeIdentity PREFIX
Function ${PREFIX}.onInit
  SetShellVarContext current
  SetRegView 64
  StrCpy $Identity "PaperCraft"
  StrCpy $VerifyToken ""
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/VERIFY=" $VerifyToken
  IfErrors identity_done
  StrLen $1 $VerifyToken
  StrCmp $1 32 0 identity_invalid
  StrCpy $1 0
  identity_character:
    StrCpy $2 $VerifyToken 1 $1
    StrCmp $2 "" identity_valid
    StrCpy $3 0
    identity_hex:
      StrCpy $4 "0123456789abcdef" 1 $3
      StrCmp $4 "" identity_invalid
      StrCmp $2 $4 identity_next
      IntOp $3 $3 + 1
      Goto identity_hex
    identity_next:
      IntOp $1 $1 + 1
      Goto identity_character
  identity_valid:
    StrCpy $Identity "PaperCraft-verify-$VerifyToken"
    Goto identity_done
  identity_invalid:
    SetErrorLevel 2
    Quit
  identity_done:
FunctionEnd
!macroend

!insertmacro InitializeIdentity ""
!insertmacro InitializeIdentity "un"

Section "PaperCraft"
  SetOutPath "$INSTDIR"
  ClearErrors
  File /r "@STAGE@\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  IfErrors install_failed
  StrCpy $UninstallCommand '$\"$INSTDIR\Uninstall.exe$\"'
  StrCmp $VerifyToken "" +2
    StrCpy $UninstallCommand '$UninstallCommand /VERIFY=$VerifyToken'
  ClearErrors
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "DisplayName" "PaperCraft"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "DisplayVersion" "@VERSION@"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "Publisher" "PaperCraft contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "DisplayIcon" "$INSTDIR\papercraft.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "UninstallString" "$UninstallCommand"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "QuietUninstallString" "$UninstallCommand /S"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity" "NoRepair" 1
  CreateShortcut "$SMPROGRAMS\$Identity.lnk" "$INSTDIR\papercraft.exe" "" "$INSTDIR\papercraft.exe"
  IfErrors install_failed
  Goto install_done
  install_failed:
    SetErrorLevel 1
    Abort "PaperCraft installation failed."
  install_done:
SectionEnd

Section "Uninstall"
  ; Delete only files shipped by this installer, preserving user-added files.
@REMOVE_FILES@
  Delete "$INSTDIR\Uninstall.exe"
@REMOVE_DIRECTORIES@
  Delete "$SMPROGRAMS\$Identity.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$Identity"
  RMDir "$INSTDIR"
SectionEnd
