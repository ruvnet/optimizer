Add-Type -AssemblyName System.Windows.Forms
$n = New-Object System.Windows.Forms.NotifyIcon
$n.Icon = [System.Drawing.SystemIcons]::Information
$n.BalloonTipTitle = 'RuVector Test'
$n.BalloonTipText = 'Hello from RuVector MemOpt'
$n.Visible = $true
$n.ShowBalloonTip(3000)
Start-Sleep -Seconds 4
$n.Dispose()
