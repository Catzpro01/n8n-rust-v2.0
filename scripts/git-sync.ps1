param([string]$Message = "")
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$status = git status --porcelain
if (-not $status) {
    Write-Host "[git-sync] Tidak ada perubahan yang perlu di-sync." -ForegroundColor Green
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Message)) {
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $Message = "sync(auto): update $timestamp"
}

Write-Host "[git-sync] Menambahkan perubahan (git add -A)..." -ForegroundColor Cyan
git add -A

Write-Host "[git-sync] Melakukan commit ($Message)..." -ForegroundColor Cyan
git commit -m "$Message"

Write-Host "[git-sync] Melakukan push ke GitHub..." -ForegroundColor Cyan
git push origin HEAD

Write-Host "[git-sync] Selesai! Semua perubahan telah tersimpan di GitHub." -ForegroundColor Green
