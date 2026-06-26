$ErrorActionPreference = 'Stop'

# Chocolatey install script for Jarvy.
#
# VERSION_PLACEHOLDER and SHA256_PLACEHOLDER are substituted by the
# `Update install script` step in publish-packages.yml::update-chocolatey
# at release time; do NOT commit a real version/sha here.
#
# Asset shape: cargo-packager produces an .msi for Windows; previous
# revisions of this file pointed at a `.zip` URL that the release
# matrix does not produce, causing the v0.0.3 Chocolatey submission to
# fail moderation with `404 Not Found` for
# jarvy-vVERSION_PLACEHOLDER-x86_64-pc-windows-msvc.zip. Switched to
# .msi via Install-ChocolateyPackage with -FileType msi and silent args.

$packageName = 'jarvy'
$version     = 'VERSION_PLACEHOLDER'
$url64       = "https://github.com/Cliftonz/Jarvy/releases/download/v$version/jarvy_${version}_x64_en-US.msi"
$checksum64  = 'SHA256_PLACEHOLDER'

$packageArgs = @{
    packageName    = $packageName
    fileType       = 'msi'
    url64bit       = $url64
    checksum64     = $checksum64
    checksumType64 = 'sha256'
    silentArgs     = '/quiet /norestart'
    validExitCodes = @(0, 3010, 1641)
}

Install-ChocolateyPackage @packageArgs
