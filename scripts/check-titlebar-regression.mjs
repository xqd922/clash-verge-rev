import { readFileSync } from 'node:fs'

const read = (path) => readFileSync(path, 'utf8')

const files = {
  layout: read('src/pages/_layout.tsx'),
  useWindow: read('src/hooks/use-window.ts'),
  windowContext: read('src/providers/window/window-context.ts'),
  windowProvider: read('src/providers/window/window-provider.tsx'),
  layoutViewer: read('src/components/setting/mods/layout-viewer.tsx'),
  windowRust: read('src-tauri/src/utils/resolve/window.rs'),
  capability: read('src-tauri/capabilities/migrated.json'),
}

const failures = []

const reject = (condition, message) => {
  if (condition) failures.push(message)
}

reject(
  /useWindowDecorations|decorated\s*[=}]|customTitlebar[\s\S]*decorated/.test(
    files.layout,
  ),
  'Layout must not gate the custom titlebar on window decorations state.',
)
reject(
  /useWindowDecorations|toggleDecorations|refreshDecorated/.test(
    files.useWindow,
  ),
  'Window hooks must not expose system titlebar toggles.',
)
reject(
  /decorated|toggleDecorations|refreshDecorated/.test(files.windowContext),
  'Window context must not track system titlebar decoration state.',
)
reject(
  /decorated|setDecorated|isDecorated|setDecorations|toggleDecorations|refreshDecorated/.test(
    files.windowProvider,
  ),
  'Window provider must not read or mutate system titlebar decorations.',
)
reject(
  /preferSystemTitlebar|useWindowDecorations|toggleDecorations|decorated/.test(
    files.layoutViewer,
  ),
  'Layout settings must not expose the Prefer System Titlebar switch.',
)
reject(
  /DEFAULT_DECORATIONS:\s*bool\s*=\s*true|#\[cfg\(not\(target_os = "linux"\)\)\]/.test(
    files.windowRust,
  ),
  'Tauri window defaults must not enable system decorations on any platform.',
)
reject(
  /core:window:allow-set-decorations/.test(files.capability),
  'Tauri capabilities must not allow runtime decoration changes.',
)

if (failures.length > 0) {
  console.error('Titlebar regression check failed:')
  for (const failure of failures) {
    console.error(`- ${failure}`)
  }
  process.exit(1)
}

console.log('Titlebar regression check passed.')
