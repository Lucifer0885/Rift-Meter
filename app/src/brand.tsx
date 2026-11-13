/** App name, version and logo, shared by the main window and the overlay. */

declare const __APP_VERSION__: string

export const APP_NAME = 'Riftmeter'
export const APP_VERSION = __APP_VERSION__

export function Logo({ className = 'h-6 w-6' }: { className?: string }) {
  return <img src="/logo.svg" alt="" className={`shrink-0 ${className}`} draggable={false} />
}
