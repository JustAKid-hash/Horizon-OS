<#
.SYNOPSIS
  Headless boot test: boot an ISO in QEMU, capture a screenshot, then shut down.

.DESCRIPTION
  Boots the given (or newest) ISO with WHPX acceleration and OVMF firmware, waits
  for the guest to reach its shell or desktop, captures a screenshot through the
  QEMU monitor, converts it from PPM to PNG, and powers the VM off.

  This is the automated smoke test for ISO builds: it proves the image actually
  boots without a human having to watch a window.

  The PPM -> PNG conversion is pure managed code (DeflateStream + CRC32), so it
  works on PowerShell 7 / .NET Core where System.Drawing.Common is unavailable.

.EXAMPLE
  pwsh -File scripts/test-boot.ps1

.EXAMPLE
  pwsh -File scripts/test-boot.ps1 -WaitSeconds 240 -Out D:\shots\horizon.png

.EXAMPLE
  pwsh -File scripts/test-boot.ps1 -KeepRunning    # leave the VM up afterwards
#>
[CmdletBinding()]
param(
  [string]$Iso,
  [int]$WaitSeconds = 180,
  [string]$Out,
  [string]$Memory = '4G',
  [int]$Cpus = 4,
  [int]$MonitorPort = 4455,
  [switch]$Bios,
  [switch]$KeepRunning,
  [switch]$Ocr
)

$ErrorActionPreference = 'Stop'

# --- locate QEMU and the ISO -------------------------------------------------

$repo = Split-Path -Parent $PSScriptRoot
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
if (-not (Test-Path $qemu)) { throw "QEMU not found at $qemu" }

if (-not $Iso) {
  $newest = Get-ChildItem (Join-Path $repo 'iso') -Filter *.iso -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $newest) { throw "No ISO found. Build one first, or pass -Iso." }
  $Iso = $newest.FullName
}
if (-not (Test-Path $Iso)) { throw "ISO not found: $Iso" }

if (-not $Out) {
  $name = [IO.Path]::GetFileNameWithoutExtension($Iso)
  $Out = Join-Path $repo "iso\boot-test-$name.png"
}
$ppm = [IO.Path]::ChangeExtension($Out, '.ppm')

# --- PPM -> PNG conversion (QEMU screendump emits PPM) ----------------------
# Pure managed encoder: no System.Drawing, no external tools.

if (-not ('PpmConverter' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.IO.Compression;

public static class PpmConverter
{
    private static readonly uint[] CrcTable = BuildCrcTable();

    private static uint[] BuildCrcTable()
    {
        var t = new uint[256];
        for (uint n = 0; n < 256; n++)
        {
            uint c = n;
            for (int k = 0; k < 8; k++)
                c = ((c & 1) != 0) ? (0xEDB88320u ^ (c >> 1)) : (c >> 1);
            t[n] = c;
        }
        return t;
    }

    private static uint Crc32(byte[] data, int offset, int count)
    {
        uint c = 0xFFFFFFFFu;
        for (int i = offset; i < offset + count; i++)
            c = CrcTable[(c ^ data[i]) & 0xFF] ^ (c >> 8);
        return c ^ 0xFFFFFFFFu;
    }

    private static uint Adler32(byte[] data)
    {
        uint a = 1, b = 0;
        foreach (byte v in data)
        {
            a = (a + v) % 65521u;
            b = (b + a) % 65521u;
        }
        return (b << 16) | a;
    }

    private static void Be32(Stream s, uint v)
    {
        s.WriteByte((byte)(v >> 24));
        s.WriteByte((byte)(v >> 16));
        s.WriteByte((byte)(v >> 8));
        s.WriteByte((byte)v);
    }

    private static void Chunk(Stream s, string type, byte[] data)
    {
        Be32(s, (uint)data.Length);
        var td = new byte[4 + data.Length];
        for (int i = 0; i < 4; i++) td[i] = (byte)type[i];
        Buffer.BlockCopy(data, 0, td, 4, data.Length);
        s.Write(td, 0, td.Length);
        Be32(s, Crc32(td, 0, td.Length));
    }

    public static void ToPng(string ppmPath, string pngPath)
    {
        byte[] b = File.ReadAllBytes(ppmPath);
        int pos = 0;

        Func<string> token = () =>
        {
            while (pos < b.Length && (b[pos] == 32 || b[pos] == 10 || b[pos] == 13 || b[pos] == 9)) pos++;
            int s = pos;
            while (pos < b.Length && !(b[pos] == 32 || b[pos] == 10 || b[pos] == 13 || b[pos] == 9)) pos++;
            return System.Text.Encoding.ASCII.GetString(b, s, pos - s);
        };

        string magic = token();
        if (magic != "P6") throw new Exception("Unsupported PPM magic: " + magic);
        int w = int.Parse(token());
        int h = int.Parse(token());
        token();          // maxval
        pos++;            // single whitespace byte before the pixel data

        // Raw scanlines, each prefixed with filter type 0.
        var raw = new byte[h * (1 + w * 3)];
        int o = 0;
        for (int y = 0; y < h; y++)
        {
            raw[o++] = 0;
            int src = pos + y * w * 3;
            Buffer.BlockCopy(b, src, raw, o, w * 3);
            o += w * 3;
        }

        byte[] zlib;
        using (var ms = new MemoryStream())
        {
            ms.WriteByte(0x78);   // zlib CMF
            ms.WriteByte(0x9C);   // zlib FLG
            using (var ds = new DeflateStream(ms, CompressionLevel.Optimal, true))
                ds.Write(raw, 0, raw.Length);
            Be32(ms, Adler32(raw));
            zlib = ms.ToArray();
        }

        var ihdr = new byte[13];
        ihdr[0] = (byte)(w >> 24); ihdr[1] = (byte)(w >> 16); ihdr[2] = (byte)(w >> 8); ihdr[3] = (byte)w;
        ihdr[4] = (byte)(h >> 24); ihdr[5] = (byte)(h >> 16); ihdr[6] = (byte)(h >> 8); ihdr[7] = (byte)h;
        ihdr[8] = 8;    // bit depth
        ihdr[9] = 2;    // colour type: truecolour RGB
        ihdr[10] = 0;   // compression
        ihdr[11] = 0;   // filter
        ihdr[12] = 0;   // interlace

        using (var fs = File.Create(pngPath))
        {
            fs.Write(new byte[] { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A }, 0, 8);
            Chunk(fs, "IHDR", ihdr);
            Chunk(fs, "IDAT", zlib);
            Chunk(fs, "IEND", new byte[0]);
        }
    }
}
'@ -ReferencedAssemblies System.IO.Compression
}

# --- build the QEMU command line --------------------------------------------

$qargs = @(
  '-M', 'q35'
  '-m', $Memory
  '-smp', $Cpus
  '-accel', 'whpx'
  '-display', 'none'
  '-vga', 'std'
  '-monitor', "tcp:127.0.0.1:$MonitorPort,server,nowait"
)

if (-not $Bios) {
  $share   = 'C:\Program Files\qemu\share'
  $code    = Join-Path $share 'edk2-x86_64-code.fd'
  $vars    = Join-Path $repo 'iso\OVMF_VARS.fd'
  if (-not (Test-Path $vars)) {
    New-Item -ItemType Directory -Force -Path (Split-Path $vars) | Out-Null
    Copy-Item (Join-Path $share 'edk2-i386-vars.fd') $vars -Force
  }
  $qargs += @(
    '-drive', "if=pflash,format=raw,readonly=on,file=$code"
    '-drive', "if=pflash,format=raw,file=$vars"
  )
}

$qargs += @('-cdrom', $Iso, '-boot', 'd')

# --- run ---------------------------------------------------------------------

$firmware = if ($Bios) { 'SeaBIOS (BIOS)' } else { 'OVMF (UEFI)' }
Write-Host ''
Write-Host "  ISO      : $Iso"
Write-Host "  Firmware : $firmware"
Write-Host "  Waiting  : $WaitSeconds s for the guest to reach its shell/desktop"
Write-Host ''

# Start-Process does not quote array elements, so any argument containing a
# space (the OVMF paths under C:\Program Files\...) must be quoted by hand.
# File redirection is handled natively by Start-Process, which also stops QEMU
# from inheriting the console handles and outliving this script.
$cmdline = ($qargs | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '

$name   = [IO.Path]::GetFileNameWithoutExtension($Iso)
$errLog = Join-Path $repo "iso\boot-test-$name.stderr.log"
$outLog = Join-Path $repo "iso\boot-test-$name.stdout.log"

$proc = Start-Process -FilePath $qemu -ArgumentList $cmdline -PassThru -NoNewWindow `
        -RedirectStandardError $errLog -RedirectStandardOutput $outLog
Write-Host "  QEMU started (pid $($proc.Id)); waiting..."

try {
  Start-Sleep -Seconds $WaitSeconds

  if ($proc.HasExited) {
    $err = if (Test-Path $errLog) { (Get-Content $errLog -Raw) } else { '(no stderr captured)' }
    throw "QEMU exited early with code $($proc.ExitCode).`n$err"
  }

  Write-Host '  Capturing screenshot via monitor...'
  $client = New-Object System.Net.Sockets.TcpClient('127.0.0.1', $MonitorPort)
  $stream = $client.GetStream()
  $writer = New-Object System.IO.StreamWriter($stream)
  $writer.AutoFlush = $true
  Start-Sleep -Milliseconds 500
  $writer.WriteLine("screendump $($ppm -replace '\\','/')")
  Start-Sleep -Seconds 4
  $writer.WriteLine('quit')
  Start-Sleep -Seconds 2
  $writer.Close(); $stream.Close(); $client.Close()

  if (-not (Test-Path $ppm)) { throw "screendump did not produce $ppm" }

  [PpmConverter]::ToPng($ppm, $Out)
  Remove-Item $ppm -Force -ErrorAction SilentlyContinue

  Write-Host ''
  Write-Host "  Screenshot: $Out"
  Write-Host ''

  if ($Ocr) {
    $ocrScript = Join-Path $PSScriptRoot 'ocr-screenshot.ps1'
    if (Test-Path $ocrScript) {
      Write-Host '  Reading the screenshot with Windows OCR...'
      Write-Host ''
      # Windows.Media.Ocr needs full-framework Windows PowerShell, not pwsh.
      & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $ocrScript -Path $Out
    }
    else {
      Write-Host "  -Ocr requested but $ocrScript is missing; skipping."
    }
  }
}
finally {
  if ($KeepRunning) {
    Write-Host "  Leaving VM running (pid $($proc.Id)). Stop it with: Stop-Process -Id $($proc.Id)"
  }
  elseif (-not $proc.HasExited) {
    Write-Host '  Shutting down VM...'
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
  }
}
