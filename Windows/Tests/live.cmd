@echo off
rem Dev loop in the VM (start from Win+R: Z:\Sevi\Windows\Tests\live.cmd): restart the freshly built Sevi, run the live test.
taskkill /IM Sevi.exe /F >nul 2>&1
start "" C:\vk\build\App\Sevi.exe
timeout /t 3 /nobreak >nul
call Z:\Sevi\Windows\Tests\run_livetest.cmd
