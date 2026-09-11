@echo off
title Soundpad
cd /d "%~dp0.."

if exist "target\release\soundpad.exe" (
    start "" "target\release\soundpad.exe"
) else (
    echo Release binary not found. Building...
    cargo build --release
    if errorlevel 1 (
        echo.
        echo [ERROR] Build failed.
        pause
        exit /b 1
    )
    start "" "target\release\soundpad.exe"
)
