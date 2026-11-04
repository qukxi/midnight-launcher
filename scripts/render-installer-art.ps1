param(
  [Parameter(Mandatory = $true)][string]$ProjectRoot,
  [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$palette = Get-Content -LiteralPath (Join-Path $OutputDirectory 'palette.json') -Raw | ConvertFrom-Json

function Color([string]$hex) { [Drawing.ColorTranslator]::FromHtml($hex) }
function Brush([string]$hex) { [Drawing.SolidBrush]::new((Color $hex)) }
function RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
  $p = [Drawing.Drawing2D.GraphicsPath]::new()
  $d = $r * 2
  $p.AddArc($x, $y, $d, $d, 180, 90)
  $p.AddArc(($x + $w - $d), $y, $d, $d, 270, 90)
  $p.AddArc(($x + $w - $d), ($y + $h - $d), $d, $d, 0, 90)
  $p.AddArc($x, ($y + $h - $d), $d, $d, 90, 90)
  $p.CloseFigure()
  return ,$p
}
function Moon($graphics, [single]$x, [single]$y, [single]$size, [string]$background) {
  $light = Brush $palette.rose
  $dark = Brush $background
  $graphics.FillEllipse($light, $x, $y, $size, $size)
  $graphics.FillEllipse($dark, ($x + $size * .34), ($y - $size * .12), ($size * .86), ($size * .86))
  $light.Dispose()
  $dark.Dispose()
}

$fonts = [Drawing.Text.PrivateFontCollection]::new()
$fonts.AddFontFile((Join-Path $ProjectRoot 'public/fonts/Jersey10-Regular.ttf'))
$pixel = [Drawing.Font]::new($fonts.Families[0], 72, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$small = [Drawing.Font]::new('Segoe UI', 27, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$tiny = [Drawing.Font]::new('Segoe UI', 24, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$ink = Brush $palette.roseLight
$muted = Brush $palette.muted
$line = [Drawing.Pen]::new((Color $palette.border), 1.5)
$version = [IO.File]::ReadAllText((Join-Path $OutputDirectory 'version.txt')).Trim()

$bitmap = [Drawing.Bitmap]::new(492, 942, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [Drawing.Graphics]::FromImage($bitmap)
$g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$background = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.Rectangle]::new(0, 0, 492, 942), (Color $palette.surface), (Color $palette.deep), 90)
$g.FillRectangle($background, 0, 0, 492, 942)
$g.DrawEllipse($line, -140, 130, 710, 710)
$g.DrawEllipse($line, -90, 180, 610, 610)
Moon $g 48 48 40 $palette.surface
$g.DrawString('M I D N I G H T', $small, $ink, 110, 52)

$image = [Drawing.Image]::FromFile((Join-Path $ProjectRoot 'public/reference/app-icon.png'))
$clip = RoundedPath 72 205 348 348 48
$g.SetClip($clip)
$g.DrawImage($image, [Drawing.Rectangle]::new(72, 205, 348, 348))
$g.ResetClip()
$g.DrawPath($line, $clip)
$g.DrawString('MIDNIGHT', $pixel, $ink, 48, 640)
$g.DrawString('L A U N C H E R', $small, $muted, 52, 728)
$g.DrawLine($line, 48, 824, 444, 824)
$g.DrawString('JAVA  /  HORIZON', $tiny, $muted, 48, 850)
$g.DrawString(('v' + $version + '   /   WINDOWS x64'), $tiny, $muted, 48, 882)
$bitmap.Save((Join-Path $OutputDirectory 'sidebar.bmp'), [Drawing.Imaging.ImageFormat]::Bmp)
$image.Dispose()
$clip.Dispose()
$background.Dispose()
$g.Dispose()
$bitmap.Dispose()

$header = [Drawing.Bitmap]::new(450, 171, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [Drawing.Graphics]::FromImage($header)
$g.Clear((Color $palette.background))
$g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$headerFont = [Drawing.Font]::new($fonts.Families[0], 46, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$g.DrawString('MIDNIGHT', $headerFont, $ink, 38, 58)
Moon $g 336 58 54 $palette.background
$header.Save((Join-Path $OutputDirectory 'header.bmp'), [Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose()
$header.Dispose()
$headerFont.Dispose()
$pixel.Dispose()
$small.Dispose()
$tiny.Dispose()
$ink.Dispose()
$muted.Dispose()
$line.Dispose()
$fonts.Dispose()
