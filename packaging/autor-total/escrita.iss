; Autor Total Escrita: instalador para Windows (Inno Setup 6). Instala só para a pessoa (sem administrador),
; cria o atalho na Área de Trabalho e no Iniciar e oferece "Abrir com" para .docx, .doc, .odt, .rtf, .md e .txt.
#ifndef Versao
  #define Versao "1.0.0"
#endif
#ifndef Pasta
  #define Pasta "..\..\target\release"
#endif

[Setup]
AppId={{6E3C7D52-6A3F-4B0E-9C1B-A70E5C41A001}
AppName=Autor Total Escrita
AppVersion={#Versao}
AppPublisher=AutorTotal LTDA
AppPublisherURL=https://autortotal.com.br/
AppSupportURL=https://autortotal.com.br/instalar.html
DefaultDirName={localappdata}\Programs\Autor Total Escrita
DefaultGroupName=Autor Total
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=AutorTotalEscrita-Setup
SetupIconFile=..\..\assets\app-icon\wordcraft.ico
UninstallDisplayIcon={app}\AutorTotalEscrita.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "atalho"; Description: "Criar um atalho na Área de Trabalho"; GroupDescription: "Atalhos:"

[Files]
Source: "{#Pasta}\wordcraft.exe"; DestDir: "{app}"; DestName: "AutorTotalEscrita.exe"; Flags: ignoreversion
Source: "..\..\NOTICE"; DestDir: "{app}\licencas"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}\licencas"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}\licencas"; Flags: ignoreversion
Source: "..\..\ATTRIBUTION.md"; DestDir: "{app}\licencas"; Flags: ignoreversion

[Icons]
Name: "{group}\Autor Total Escrita"; Filename: "{app}\AutorTotalEscrita.exe"
Name: "{userdesktop}\Autor Total Escrita"; Filename: "{app}\AutorTotalEscrita.exe"; Tasks: atalho

[Registry]
Root: HKCU; Subkey: "Software\Classes\Applications\AutorTotalEscrita.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Autor Total Escrita"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\AutorTotalEscrita.exe\shell\open\command"; ValueType: string; ValueData: """{app}\AutorTotalEscrita.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\AutorTotalEscrita.Documento"; ValueType: string; ValueData: "Documento"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\AutorTotalEscrita.Documento\DefaultIcon"; ValueType: string; ValueData: "{app}\AutorTotalEscrita.exe,0"
Root: HKCU; Subkey: "Software\Classes\AutorTotalEscrita.Documento\shell\open\command"; ValueType: string; ValueData: """{app}\AutorTotalEscrita.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.docx\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.doc\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.odt\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.rtf\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.txt\OpenWithProgids"; ValueType: string; ValueName: "AutorTotalEscrita.Documento"; ValueData: ""; Flags: uninsdeletevalue

[Run]
Filename: "{app}\AutorTotalEscrita.exe"; Description: "Abrir o Autor Total Escrita"; Flags: nowait postinstall skipifsilent
