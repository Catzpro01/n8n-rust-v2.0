@echo off
title Auto-Sync GitHub n8n-rust-v2.0
echo Memulai Auto-Sync GitHub untuk n8n-rust-v2.0...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\auto-git-watcher.ps1"
pause
