import { createBrowserRouter, Navigate, type RouteObject } from 'react-router'

import Layout from './_layout'
import { navItems } from './_navigation'

export const router = createBrowserRouter([
  {
    path: '/',
    Component: Layout,
    children: [
      { index: true, element: <Navigate to="/proxies" replace /> },
      ...navItems.map(
        (item) =>
          ({
            path: item.path,
            Component: item.Component,
          }) as RouteObject,
      ),
    ],
  },
])
