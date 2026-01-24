@echo off
echo Creating portable package...

set VERSION=1.0.0
set DIST=..\dist
set APP=RuVectorMemOpt

mkdir %DIST% 2>nul

echo Copying files...
copy ..\target\release\ruvector-memopt.exe %DIST%\%APP%.exe
copy ..\README.md %DIST%\

echo Creating quick-start script...
(
echo @echo off
echo echo RuVector Memory Optimizer v%VERSION%
echo echo.
echo echo Commands:
echo echo   %APP% status    - Check memory
echo echo   %APP% optimize  - Free memory now
echo echo   %APP% tray      - System tray icon
echo echo   %APP% cpu       - CPU info
echo echo   %APP% dashboard - Real-time view
echo echo.
echo pause
) > %DIST%\README.bat

echo.
echo Done! Files in %DIST%\
pause
