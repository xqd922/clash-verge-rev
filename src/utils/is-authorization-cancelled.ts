/** True when the user dismissed the system authorization prompt. */
export const isAuthorizationCancelled = (error: unknown) => {
  const detail =
    typeof error === 'string'
      ? error
      : error instanceof Error
        ? error.message
        : typeof error === 'object' &&
            error !== null &&
            'detail' in error &&
            typeof error.detail === 'string'
          ? error.detail
          : ''
  const text = detail.toLowerCase()
  return (
    text.includes('user canceled') ||
    text.includes('user cancelled') ||
    text.includes('error_cancelled') ||
    text.includes('os error 1223') ||
    text.includes('(-128)')
  )
}
