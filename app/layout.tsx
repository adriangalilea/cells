import type { Metadata, Viewport } from "next"
import { Geist, Geist_Mono } from "next/font/google"
import { ThemeProvider } from "next-themes"
import { PRODUCT } from "@/lib/product"
import "./globals.css"

const geist = Geist({ subsets: ["latin"], variable: "--font-geist" })
const mono = Geist_Mono({ subsets: ["latin"], variable: "--font-geist-mono" })

export const metadata: Metadata = {
  metadataBase: new URL(`https://${PRODUCT.domain}`),
  title: `${PRODUCT.name}: ${PRODUCT.tagline.replace(/\.$/, "")}`,
  description: `A ${PRODUCT.extension} file is a table whose header carries its types and its formulas, which imports other files and can be filled by a script. Plain text, checked by every reader.`,
}

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: dark)", color: "#0b0d11" },
    { media: "(prefers-color-scheme: light)", color: "#fafbfc" },
  ],
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${geist.variable} ${mono.variable}`} suppressHydrationWarning>
      <body>
        <ThemeProvider attribute="class" defaultTheme="dark" enableSystem={false} disableTransitionOnChange>
          <div className="ground" aria-hidden />
          {children}
        </ThemeProvider>
      </body>
    </html>
  )
}
