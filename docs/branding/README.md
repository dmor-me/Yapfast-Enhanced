# Enhanced branding

The current logo is **Ribbon E**. It retains the original Yapfast starburst and
palette: `#FFBF47` gold and `#1D1607` dark lettering.

Active sources are `packaging/icon.svg`, `ui/icons/logo.svg` and
`packaging/windows/branding/yapfast-enhanced.svg`. App, taskbar, tray,
notifications, installer, uninstaller and Repair icons derive from these marks.
The standalone E in `ui/icons/enhanced.svg` identifies Enhanced settings.
The Windows ICO contains 16/20/24/32/40/48/64/128/256 pixel entries.

Run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/update-enhanced-branding.ps1`
from the repository root to regenerate the SVG, PNG, ICO and installer BMP assets.
The path data and Windows renderer are in `scripts/EnhancedBranding.cs`.
The installer sidebar labels are centered below the logo.
