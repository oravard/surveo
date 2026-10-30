@echo off
rem Prototype 5 (IMPL.md, 5.1) : mesures du classifieur sur ce poste.
rem Les résultats sont écrits dans resultats.txt, à renvoyer.
cd /d "%~dp0"
chcp 65001 >nul
(
  echo === Poste
  powershell -NoProfile -Command "Get-CimInstance Win32_Processor | Format-List Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed; '{0:N1} Go de mémoire' -f ((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)"
  ver
  echo === Runtime Visual C++
  where vcruntime140.dll vcruntime140_1.dll msvcp140.dll msvcp140_1.dll 2>&1
  echo === Exemples
  classifieur.exe "Bonne journée à tous, on se voit demain en TP." "Je n'en peux plus, j'ai envie de disparaître." "T'es vraiment qu'un gros nul, personne ne veut de toi ici."
  echo === Validation, 1 fil
  classifieur.exe --threads 1
  echo === Validation, 2 fils
  classifieur.exe --threads 2 --limit 1000
  echo === Poids non signés, 1 fil
  classifieur.exe --model model-uint8.onnx --limit 1000
) > resultats.txt 2>&1
type resultats.txt
pause
