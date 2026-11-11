# Read a PNG screenshot with the OCR engine built into Windows.
# Uses Windows PowerShell 5.1 (full .NET Framework) for System.Drawing upscaling,
# plus the WinRT Windows.Media.Ocr API.
param(
  [Parameter(Mandatory = $true)][string]$Path,
  [int]$Scale = 3
)

$ErrorActionPreference = 'Stop'

# --- upscale: OCR is much more reliable on larger glyphs --------------------
Add-Type -AssemblyName System.Drawing

$src = [System.Drawing.Image]::FromFile($Path)
$srcW = $src.Width
$srcH = $src.Height
$w = $srcW * $Scale
$h = $srcH * $Scale
$big = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($big)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
$g.DrawImage($src, 0, 0, $w, $h)
$g.Dispose()
$bigPath = [IO.Path]::ChangeExtension($Path, '.ocr.png')
$big.Save($bigPath, [System.Drawing.Imaging.ImageFormat]::Png)
$src.Dispose(); $big.Dispose()

Write-Host "source  : $Path ($srcW x $srcH)"
Write-Host "upscaled: $bigPath ($w x $h)"

# --- WinRT plumbing ---------------------------------------------------------
Add-Type -AssemblyName System.Runtime.WindowsRuntime

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and
  $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]

function Await($op, $type) {
  $m = $asTaskGeneric.MakeGenericMethod($type)
  $t = $m.Invoke($null, @($op))
  $t.Wait(-1) | Out-Null
  $t.Result
}

[Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
[Windows.Storage.FileAccessMode, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType = WindowsRuntime] | Out-Null
[Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null

# --- pick an OCR engine -----------------------------------------------------
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
if (-not $engine) {
  $langs = [Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages
  Write-Host "user-profile engine unavailable; available languages: $($langs | ForEach-Object { $_.LanguageTag })"
  if ($langs.Count -gt 0) { $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($langs[0]) }
}
if (-not $engine) { throw 'No OCR engine available on this system.' }
Write-Host "engine  : $($engine.RecognizerLanguage.LanguageTag)"

# --- run OCR ----------------------------------------------------------------
$file    = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($bigPath)) ([Windows.Storage.StorageFile])
$stream  = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap  = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
$result  = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])

Write-Host ''
Write-Host '--- OCR TEXT ---'
foreach ($line in $result.Lines) { Write-Host $line.Text }
Write-Host '--- END ---'
