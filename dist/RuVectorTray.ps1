# Launch RuVector Tray completely hidden
$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$exePath = Join-Path $scriptPath "ruvector-memopt-tray.exe"
Start-Process -FilePath $exePath -WindowStyle Hidden
