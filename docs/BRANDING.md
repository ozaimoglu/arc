# Arc brand — Eclipse

Arc 0.1.8 uses a faceted platinum A with a warm gold arc and a custom geometric ARC wordmark. The dark Windows tile has a subtle edge and transparent corners. The silhouette carries the identity at small sizes; metallic shading adds depth at larger sizes.

## Assets

| File | Use |
| --- | --- |
| `public/arc-mark.svg` | Editable transparent emblem; shared source for all variants |
| `public/arc-wordmark.svg` | Editable ARC lettering drawn as paths; no font dependency |
| `public/arc-logo.svg` | Transparent horizontal logo; application header |
| `public/arc-logo.png` | High resolution transparent logo export |
| `public/arc.svg` | Square SVG icon; browser favicon |
| `public/arc-icon.png` | 1024px transparent-corner Windows icon export |
| `src-tauri/icons/icon.png` | 256px native application icon |
| `src-tauri/icons/icon.ico` | Windows executable, shortcut, installer and uninstaller icon |

The ICO contains actual 16, 24, 32, 48, 64, 128 and 256px images. The header uses a 116 × 38px SVG logo (104 × 34px in compact windows) in the existing `Arc home` button. Its decorative image is hidden from screen readers by an empty alt text; the button retains its accessible name, navigation action and focus ring.

## Rebuild

Install the project's npm dependencies and Python Pillow, then run:

```powershell
python scripts/generate-icons.py
```

The script combines the two editable vector sources, uses the project's installed Tauri SVG renderer, and exports the native PNG/ICO plus public assets. Icon paths are never redrawn separately. Temporary renderer output is cleaned up by Python's temporary-directory context.

The gold accent belongs to the brand mark. Existing control colors, rating colors, typography and cinematic library composition continue to use the app's design tokens. SVG proportions remain fixed, and hover only changes opacity.
