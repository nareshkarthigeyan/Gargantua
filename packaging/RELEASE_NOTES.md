A scripted, interactive black-hole descent with real-time Schwarzschild ray tracing.

### Downloads

- **macOS DMG**: open the disk image, drag Gargantua onto Applications, then open it. The filename identifies the supported Mac architecture.
- **Windows x64 setup EXE**: install for your user, create shortcuts, and launch from the final setup screen. Includes an uninstaller.
- **Windows x64 portable ZIP**: extract and open Gargantua.exe. Includes license notices.
- **Windows standalone EXE**: the same self-contained app executable, without an installer.

No Rust toolchain or development libraries are required. A compatible GPU and its normal system drivers are required (Metal on macOS, DirectX 12 or Vulkan on Windows).

### In this release

- Deterministic spiral with unrestricted mouse/trackpad look.
- Warm orange glow that scales with apparent source size.
- Two-second visual-novel dialogue and an immersive mode that hides all text.
- Earth/your-time comparison with a sharply increasing near-horizon ratio. Clock precision extends beyond the finite graphical cutoff.
- New black-hole app icon and native desktop packages.

Controls: move the mouse/trackpad to look; I toggles immersive mode; Esc opens the menu; X recenters; Space pauses; R replays.

### Installation notes

These independent builds are not backed by paid code-signing certificates. The macOS app is ad-hoc signed, not Apple notarized; macOS may require approval through Privacy & Security. Windows may show an unknown-publisher/SmartScreen prompt. Verify that you downloaded from this repository and compare SHA256SUMS.txt if needed.

The personal-use music recording is not included. The public builds run the same timed journey without that recording. The macOS renderer has been visually tested; Windows builds are compiled and installer/launch/uninstaller smoke-tested in CI, without full interactive GPU validation.
