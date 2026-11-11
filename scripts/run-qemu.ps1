<#
.SYNOPSIS
  Boot a Horizon OS / Arch ISO under QEMU with WHPX acceleration.

.DESCRIPTION
  Defaults to UEFI boot via the OVMF firmware shipped with QEMU for Windows. The
  firmware code image is used read-only; a writable copy of the variable store is
  kept in the repo's iso/ directory, since the original lives under Program Files.

.EXAMPLE
  pwsh -File scripts/run-qemu.ps1

.EXAMPLE
  pwsh -File scripts/run-qemu.ps1 -Memory 8G -Cpus 6

.EXAMPLE
  pwsh -File scripts/run-qemu.ps1 -Bios

.EXAMPLE
  pwsh -File scripts/run-qemu.ps1 -Disk D:\vms\horizon.qcow2
#>
[CmdletBinding()]
param(
  [string]$Iso,
  [string]$Memory = '4G',
  [int]$Cpus = 4,
  [string]$Disk,
  [switch]$Bios,
  [switch]$NoAccel,
  [string]$Display = 'gtk'
)

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
if (-not (Test-Path $qemu)) { throw "QEMU not found at $qemu" }

if (-not $Iso) {
  $isoDir = Join-Path $repo 'iso'
  $newest = Get-ChildItem $isoDir -Filter *.iso -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
  if (-not $newest) {
    throw "No ISO found in $isoDir. Run scripts/build-iso.sh first, or pass -Iso."
  }
  $Iso = $newest.FullName
}
if (-not (Test-Path $Iso)) { throw "ISO not found: $Iso" }

$qargs = @(
  '-M', 'q35'
  '-m', $Memory
  '-smp', $Cpus
  '-device', 'virtio-vga'
)

if ($Display -ne 'auto') { $qargs += @('-display', $Display) }
if (-not $NoAccel) { $qargs += @('-accel', 'whpx') }

if (-not $Bios) {
  $share   = 'C:\Program Files\qemu\share'
  $code    = Join-Path $share 'edk2-x86_64-code.fd'
  $varsSrc = Join-Path $share 'edk2-i386-vars.fd'
  $vars    = Join-Path $repo 'iso\OVMF_VARS.fd'

  if (-not (Test-Path $code)) { throw "OVMF firmware not found: $code" }
  if (-not (Test-Path $vars)) {
    New-Item -ItemType Directory -Force -Path (Split-Path $vars) | Out-Null
    Copy-Item $varsSrc $vars -Force
    Write-Host "Created writable OVMF variable store: $vars"
  }

  $qargs += @(
    '-drive', "if=pflash,format=raw,readonly=on,file=$code"
    '-drive', "if=pflash,format=raw,file=$vars"
  )
}

$qargs += @('-cdrom', $Iso, '-boot', 'd')

if ($Disk) {
  if (-not (Test-Path $Disk)) { throw "Disk image not found: $Disk" }
  $qargs += @('-drive', "file=$Disk,if=virtio,format=qcow2")
}

$firmware = if ($Bios) { 'SeaBIOS (BIOS)' } else { 'OVMF (UEFI)' }
$accel    = if ($NoAccel) { 'tcg (software)' } else { 'whpx (hardware)' }

Write-Host ''
Write-Host "  ISO      : $Iso"
Write-Host "  Firmware : $firmware"
Write-Host "  Accel    : $accel"
Write-Host "  CPU/RAM  : $Cpus vCPU / $Memory"
if ($Disk) { Write-Host "  Disk     : $Disk" }
Write-Host ''
Write-Host '  Starting QEMU... (Ctrl+Alt+G releases the mouse, Ctrl+Alt+2 for the monitor)'
Write-Host ''

& $qemu @qargs
