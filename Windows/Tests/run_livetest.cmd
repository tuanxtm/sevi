@echo off
rem Runs the live typing test against the running Sevi; output to Z:\Sevi\Windows\Tests\livetest.out
C:\vk\build\App\sevi_livetest.exe C:\vk\src\Windows\Tests\cases.tsv > Z:\Sevi\Windows\Tests\livetest.out 2>&1
echo EXIT %ERRORLEVEL% >> Z:\Sevi\Windows\Tests\livetest.out
