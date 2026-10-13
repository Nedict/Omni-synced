#!/usr/bin/env bash
# Compile the ModelForge C# workspace against the SolidCraft interop and report how many errors
# are SolidWorks-API errors (must be 0) versus WinForms/GUI errors (expected: the GUI toolkit is
# Windows-only and unrelated to driving SolidCraft).
#
# Usage: tools/check-workspace-compiles.sh [WORKSPACE_BUILD_DIR]
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
WORKSPACE="${1:-/Users/kevin/Desktop/parallels-recovery/qemu/archive/solidworks-wine-repro/qemu/runtime/solidworks-wine/workspace-builds/64166479807f}"
OUT="${TMPDIR:-/tmp}/solidcraft-workspace-compile"

export DOTNET_ROOT="${DOTNET_ROOT:-/opt/homebrew/opt/dotnet/libexec}"
PATH="$DOTNET_ROOT:$PATH"

if [[ ! -d "$WORKSPACE" ]]; then
    echo "check-workspace-compiles: no such workspace build: $WORKSPACE" >&2
    exit 1
fi

# Build the interop assembly first.
dotnet build "$ROOT/csharp/SolidCraft.Interop" -v q >/dev/null

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$WORKSPACE"/*.cs "$OUT"/

cat > "$OUT/probe.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <LangVersion>7.3</LangVersion>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
    <Nullable>disable</Nullable>
    <NoWarn>CS0108;CS0618;CS0649;CS0169;CS0414;CS0067</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <Compile Include="*.cs" />
    <Reference Include="SolidCraft.Interop">
      <HintPath>$ROOT/csharp/SolidCraft.Interop/bin/Debug/net10.0/SolidCraft.Interop.dll</HintPath>
    </Reference>
  </ItemGroup>
</Project>
EOF

LOG="$OUT/build.log"
( cd "$OUT" && dotnet build -v q 2>&1 ) > "$LOG" || true

total=$(grep -cE "error CS" "$LOG" || true)
gui=$(grep -E "error CS" "$LOG" | grep -ciE "System.Windows|Forms|Control|ComboBox|TextBox|Label|PaintEventArgs|FlowLayoutPanel|Button|TabControl|ListBox|Form|Graphics|TreeView|TreeNode|ToolTip|Timer|PropertyGrid|Panel|NumericUpDown|GroupBox|CheckBox|MouseEventArgs|Drawing" || true)
api=$((total - gui))

echo "workspace:            $WORKSPACE"
echo "total errors:         $total"
echo "WinForms/Drawing:     $gui   (expected; Windows-only GUI toolkit)"
echo "SolidWorks-API errors: $api   (must be 0)"

if [[ "$api" -ne 0 ]]; then
    echo
    echo "SolidWorks-API errors (these must be fixed):"
    grep -E "error CS" "$LOG" | grep -viE "System.Windows|Forms|Control|ComboBox|TextBox|Label|PaintEventArgs|FlowLayoutPanel|Button|TabControl|ListBox|Form|Graphics|TreeView|TreeNode|ToolTip|Timer|PropertyGrid|Panel|NumericUpDown|GroupBox|CheckBox|MouseEventArgs|Drawing" | sort -u | head -40
    exit 1
fi
echo "OK: the workspace's SolidWorks API usage compiles against the SolidCraft interop."
