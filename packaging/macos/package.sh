#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
arch=${1:-$(uname -m)}
case "$arch" in arm64|aarch64) arch=arm64 ;; x86_64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; esac
name="Gargantua-$version-macos-$arch"
stage="target/package-$arch"
app="$stage/Gargantua.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses" dist
cp target/release/gargantua "$app/Contents/MacOS/Gargantua"
cp assets/icons/gargantua.icns "$app/Contents/Resources/Gargantua.icns"
cp LICENSE "$app/Contents/Resources/licenses/LICENSE"
cp assets/fonts/OFL.txt "$app/Contents/Resources/licenses/OFL.txt"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Gargantua</string>
<key>CFBundleDisplayName</key><string>Gargantua</string>
<key>CFBundleIdentifier</key><string>io.github.nareshkarthigeyan.gargantua</string>
<key>CFBundleExecutable</key><string>Gargantua</string>
<key>CFBundleIconFile</key><string>Gargantua.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>LSApplicationCategoryType</key><string>public.app-category.games</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
# Ad-hoc signing seals the bundle; Developer ID/notarization requires owner's credentials.
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"
ln -sfn /Applications "$stage/Applications"
cp packaging/macos/INSTALL.txt "$stage/INSTALL.txt"
hdiutil create -volname "Gargantua" -srcfolder "$stage" -ov -format UDZO "dist/$name.dmg"
hdiutil verify "dist/$name.dmg"
