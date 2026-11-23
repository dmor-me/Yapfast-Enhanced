# Rebuild all Enhanced marks using the original exact gold and selected ribbon E.
$ErrorActionPreference='Stop'
$brandRoot=Split-Path -Parent $PSScriptRoot
$brandDir=Join-Path $brandRoot 'packaging\windows\branding'
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -Path (Join-Path $PSScriptRoot 'EnhancedBranding.cs')
$brandSvg=@"
<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="12 12 78 78">
  <path fill="#ffbf47" stroke="#ffbf47" stroke-width="3" stroke-linejoin="round" d="$([EnhancedBrand]::Star)" />
  <path fill="#1d1607" transform="translate(2 -1.5)" d="$([EnhancedBrand]::Ribbon)" />
</svg>
"@
[IO.File]::WriteAllText((Join-Path $brandRoot 'packaging\icon.svg'),$brandSvg)
[IO.File]::WriteAllText((Join-Path $brandDir 'yapfast-enhanced.svg'),$brandSvg)
$brandMask=@"
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="12 12 78 78">
  <mask id="mark">
    <path fill="#fff" stroke="#fff" stroke-width="3" stroke-linejoin="round" d="$([EnhancedBrand]::Star)" />
    <path fill="#000" transform="translate(2 -1.5)" d="$([EnhancedBrand]::Ribbon)" />
  </mask>
  <rect x="0" y="0" width="100" height="100" fill="#fff" mask="url(#mark)" />
</svg>
"@
[IO.File]::WriteAllText((Join-Path $brandRoot 'ui\icons\logo.svg'),$brandMask)
$brandPng=[EnhancedBrand]::Render(1024)
$brandPng.Save((Join-Path $brandDir 'yapfast-enhanced.png'),[Drawing.Imaging.ImageFormat]::Png)
$brandPng.Dispose()
[EnhancedBrand]::Icon((Join-Path $brandDir 'yapfast-enhanced.ico'))
Copy-Item -LiteralPath (Join-Path $brandDir 'yapfast-enhanced.ico') -Destination (Join-Path $brandRoot 'packaging\windows\yapfast.ico')
foreach($brandSpec in @(@(164,314,'sidebar.bmp'),@(64,64,'header.bmp'))){
 $brandBmp=[Drawing.Bitmap]::new([int]$brandSpec[0],[int]$brandSpec[1],[Drawing.Imaging.PixelFormat]::Format24bppRgb)
 $brandG=[Drawing.Graphics]::FromImage($brandBmp);$brandG.Clear([Drawing.ColorTranslator]::FromHtml('#141518'))
 $brandG.InterpolationMode=[Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
 if($brandSpec[2] -eq 'sidebar.bmp'){
  $brandLogo=[EnhancedBrand]::Render(132);$brandG.DrawImage($brandLogo,16,35,132,132)
  $brandFormat=[Drawing.StringFormat]::new();$brandFormat.Alignment=[Drawing.StringAlignment]::Center
  $brandGold=[Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#FFBF47'))
  $brandFont=[Drawing.Font]::new('Segoe UI',16,[Drawing.FontStyle]::Bold);$brandSubFont=[Drawing.Font]::new('Segoe UI',13)
  $brandG.DrawString('Yapfast',$brandFont,$brandGold,[Drawing.RectangleF]::new(0,186,164,30),$brandFormat)
  $brandG.DrawString('Enhanced',$brandSubFont,[Drawing.Brushes]::White,[Drawing.RectangleF]::new(0,216,164,28),$brandFormat)
  $brandFormat.Dispose();$brandGold.Dispose();$brandFont.Dispose();$brandSubFont.Dispose();$brandLogo.Dispose()
 }else{$brandLogo=[EnhancedBrand]::Render(58);$brandG.DrawImage($brandLogo,3,3,58,58);$brandLogo.Dispose()}
 $brandBmp.Save((Join-Path $brandDir $brandSpec[2]),[Drawing.Imaging.ImageFormat]::Bmp)
 $brandG.Dispose();$brandBmp.Dispose()
}
Write-Output 'Rebuilt exact-color SVG, PNG, multi-size ICO and centered installer artwork.'
