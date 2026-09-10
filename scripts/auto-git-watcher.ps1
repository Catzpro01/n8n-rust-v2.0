param([int]$IntervalSeconds = 15)
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

Write-Host "==================================================" -ForegroundColor Yellow
Write-Host " AUTO-SYNC GITHUB AKTIF (Interval: $IntervalSeconds detik)" -ForegroundColor Yellow
Write-Host " Folder: $repoRoot" -ForegroundColor Yellow
Write-Host " Repo: Catzpro01/n8n-rust-v2.0" -ForegroundColor Yellow
Write-Host " Tekan Ctrl+C untuk berhenti" -ForegroundColor Yellow
Write-Host "==================================================" -ForegroundColor Yellow

while ($true) {
    $status = git status --porcelain
    if ($status) {
        $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        Write-Host "[$timestamp] Perubahan terdeteksi! Sinkronisasi ke GitHub..." -ForegroundColor Cyan
        git add -A
        git commit -m "auto(sync): update $timestamp"
        Write-Host "[$timestamp] Sinkronisasi selesai." -ForegroundColor Green
    }
    Start-Sleep -Seconds $IntervalSeconds
}
