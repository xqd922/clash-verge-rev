import { readFileSync } from 'node:fs'

const read = (path) => readFileSync(path, 'utf8')

const files = {
  package: read('package.json'),
  lockfile: read('pnpm-lock.yaml'),
  globalStyles: read('src/assets/styles/index.scss'),
  theme: read('src/pages/_layout/hooks/use-custom-theme.ts'),
  layoutStyles: read('src/assets/styles/layout.scss'),
  layoutItem: read('src/components/layout/layout-item.tsx'),
  windowControls: read('src/components/layout/window-controller.tsx'),
  routes: read('src/pages/_routers.tsx'),
  proxyMini: read('src/components/proxy/proxy-item-mini.tsx'),
  proxyRender: read('src/components/proxy/proxy-render.tsx'),
  profileItem: read('src/components/profile/profile-item.tsx'),
}

const failures = []

const requireText = (file, text, message) => {
  if (!file.includes(text)) failures.push(message)
}

const rejectText = (file, text, message) => {
  if (file.includes(text)) failures.push(message)
}

requireText(
  files.globalStyles,
  "-apple-system, BlinkMacSystemFont, 'Segoe UI'",
  'The personal system font stack changed.',
)
requireText(
  files.theme,
  "shadows: Array(25).fill('none')",
  'The shadow-free personal MUI theme changed.',
)
requireText(
  files.theme,
  'fontFamily: setting.font_family',
  'Custom font-family support changed.',
)
requireText(
  files.layoutItem,
  "fontWeight: '700'",
  'Navigation labels must keep the personal 700 font weight.',
)
requireText(
  files.layoutItem,
  "padding: '4px 0px'",
  'Navigation item density changed.',
)
requireText(
  files.layoutStyles,
  'height: 36px',
  'The personal 36px titlebar contract changed.',
)
requireText(
  files.windowControls,
  'width: 46',
  'Windows caption buttons must remain 46px wide.',
)
requireText(
  files.windowControls,
  'borderRadius: 0',
  'Windows caption buttons must remain square.',
)
requireText(
  files.windowControls,
  '#e81123',
  'The native-style Windows close-button hover color changed.',
)
requireText(
  files.routes,
  "path: '/'",
  'The proxies-first personal route changed.',
)
rejectText(files.routes, "path: '/home'", 'The Home route was reintroduced.')
requireText(
  files.proxyMini,
  'height: 56',
  'Mini proxy items must remain 56px high.',
)
requireText(
  files.proxyRender,
  "borderRadius: '8px'",
  'Proxy card radius changed.',
)
requireText(
  files.profileItem,
  "fontSize: '18px'",
  'Profile title size changed.',
)
requireText(
  files.profileItem,
  "fontWeight: '600'",
  'Profile title weight changed.',
)
requireText(
  files.package,
  '"@mui/material": "^9.0.0"',
  'The personal MUI version range changed without visual approval.',
)
requireText(
  files.package,
  '"react": "19.2.5"',
  'The personal React version changed without visual approval.',
)
requireText(
  files.lockfile,
  "'@mui/material@9.0.0'",
  'The lockfile no longer pins the personal MUI 9.0.0 visual baseline.',
)

if (failures.length > 0) {
  console.error('Personal UI regression check failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log('Personal UI regression check passed.')
