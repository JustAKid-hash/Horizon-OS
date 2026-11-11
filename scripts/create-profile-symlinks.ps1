<#
.SYNOPSIS
  Create the symlinks the Horizon archiso profile needs inside airootfs.

.DESCRIPTION
  The profile ships ~27 symlinks (systemd unit enablement, /etc/localtime,
  /etc/resolv.conf, and a /dev/null mask). Git stores them as mode 120000, which
  only restores as real symlinks when `core.symlinks=true` and the machine either
  has Developer Mode on or is running elevated.

  If a checkout ever lands without them (a fresh clone on a machine with symlink
  support off, or a zip download), run this script to recreate them. It is
  idempotent: existing links are left alone unless -Force is given.

.EXAMPLE
  pwsh -File scripts/create-profile-symlinks.ps1
  pwsh -File scripts/create-profile-symlinks.ps1 -Force
#>
[CmdletBinding()]
param(
  [string]$Profile = (Join-Path (Split-Path -Parent $PSScriptRoot) 'profile'),
  [switch]$Force
)

$ErrorActionPreference = 'Stop'

$airootfs = Join-Path $Profile 'airootfs'
if (-not (Test-Path $airootfs)) { throw "airootfs not found under $Profile" }

# path relative to airootfs  ->  link target (exactly as the kernel sees it)
$links = [ordered]@{
  'etc/localtime'                                                                   = '/usr/share/zoneinfo/UTC'
  'etc/resolv.conf'                                                                 = '/run/systemd/resolve/stub-resolv.conf'

  # Mask systemd's GPT auto-generator: on a live medium it must not try to mount
  # whatever happens to be on the disk.
  'etc/systemd/system-generators/systemd-gpt-auto-generator'                         = '/dev/null'

  # D-Bus aliases
  'etc/systemd/system/dbus-org.freedesktop.ModemManager1.service'                    = '/usr/lib/systemd/system/ModemManager.service'
  'etc/systemd/system/dbus-org.freedesktop.network1.service'                         = '/usr/lib/systemd/system/systemd-networkd.service'
  'etc/systemd/system/dbus-org.freedesktop.resolve1.service'                         = '/usr/lib/systemd/system/systemd-resolved.service'
  'etc/systemd/system/dbus-org.freedesktop.timesync1.service'                        = '/usr/lib/systemd/system/systemd-timesyncd.service'

  # multi-user.target
  'etc/systemd/system/multi-user.target.wants/ModemManager.service'                  = '/usr/lib/systemd/system/ModemManager.service'
  'etc/systemd/system/multi-user.target.wants/choose-mirror.service'                 = '../choose-mirror.service'
  'etc/systemd/system/multi-user.target.wants/hv_fcopy_daemon.service'               = '/usr/lib/systemd/system/hv_fcopy_daemon.service'
  'etc/systemd/system/multi-user.target.wants/hv_kvp_daemon.service'                 = '/usr/lib/systemd/system/hv_kvp_daemon.service'
  'etc/systemd/system/multi-user.target.wants/hv_vss_daemon.service'                 = '/usr/lib/systemd/system/hv_vss_daemon.service'
  'etc/systemd/system/multi-user.target.wants/iwd.service'                           = '/usr/lib/systemd/system/iwd.service'
  'etc/systemd/system/multi-user.target.wants/livecd-talk.service'                   = '/etc/systemd/system/livecd-talk.service'
  'etc/systemd/system/multi-user.target.wants/pacman-init.service'                   = '../pacman-init.service'
  'etc/systemd/system/multi-user.target.wants/sshd.service'                          = '/usr/lib/systemd/system/sshd.service'
  'etc/systemd/system/multi-user.target.wants/systemd-networkd.service'              = '/usr/lib/systemd/system/systemd-networkd.service'
  'etc/systemd/system/multi-user.target.wants/systemd-resolved.service'              = '/usr/lib/systemd/system/systemd-resolved.service'
  'etc/systemd/system/multi-user.target.wants/vboxservice.service'                   = '/usr/lib/systemd/system/vboxservice.service'
  'etc/systemd/system/multi-user.target.wants/vmtoolsd.service'                      = '/usr/lib/systemd/system/vmtoolsd.service'
  'etc/systemd/system/multi-user.target.wants/vmware-vmblock-fuse.service'           = '/usr/lib/systemd/system/vmware-vmblock-fuse.service'

  # other targets
  'etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service' = '/usr/lib/systemd/system/systemd-networkd-wait-online.service'
  'etc/systemd/system/sockets.target.wants/pcscd.socket'                             = '/usr/lib/systemd/system/pcscd.socket'
  'etc/systemd/system/sockets.target.wants/systemd-networkd.socket'                  = '/usr/lib/systemd/system/systemd-networkd.socket'
  'etc/systemd/system/sound.target.wants/livecd-alsa-unmuter.service'                = '../livecd-alsa-unmuter.service'
  'etc/systemd/system/sysinit.target.wants/systemd-time-wait-sync.service'           = '/usr/lib/systemd/system/systemd-time-wait-sync.service'
  'etc/systemd/system/sysinit.target.wants/systemd-timesyncd.service'                = '/usr/lib/systemd/system/systemd-timesyncd.service'
}

$created = 0; $skipped = 0; $replaced = 0

foreach ($rel in $links.Keys) {
  $linkPath = Join-Path $airootfs ($rel -replace '/', '\')
  $target   = $links[$rel]

  New-Item -ItemType Directory -Force -Path (Split-Path $linkPath) | Out-Null

  $existing = Get-Item $linkPath -Force -ErrorAction SilentlyContinue
  if ($existing) {
    if ($existing.LinkType -eq 'SymbolicLink' -and $existing.Target -eq $target) { $skipped++; continue }
    if (-not $Force) {
      Write-Warning "$rel exists and is not the expected symlink; use -Force to replace"
      $skipped++
      continue
    }
    Remove-Item $linkPath -Force -Recurse
    $replaced++
  }

  New-Item -ItemType SymbolicLink -Path $linkPath -Target $target -ErrorAction Stop | Out-Null
  $created++
}

Write-Host ''
Write-Host "  symlinks: $created created, $replaced replaced, $skipped already correct/skipped"
Write-Host "  total   : $($links.Count)"
Write-Host ''
Write-Host '  Verify git recorded them as symlinks (mode 120000):'
Write-Host "    git -C '$Profile\..' ls-files -s profile/airootfs | Select-String 120000"
