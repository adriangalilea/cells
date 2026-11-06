import type { Metadata, Viewport } from "next"
import { Geist, Geist_Mono } from "next/font/google"
import { ThemeProvider } from "next-themes"
import { PRODUCT } from "@/lib/product"
import "./globals.css"

const geist = Geist({ subsets: ["latin"], variable: "--font-geist" })
const mono = Geist_Mono({ subsets: ["latin"], variable: "--font-geist-mono" })

const TITLE = `${PRODUCT.name}: ${PRODUCT.tagline.replace(/\.$/, "")}`

export const metadata: Metadata = {
  metadataBase: new URL(`https://${PRODUCT.domain}`),
  title: TITLE,
  description: PRODUCT.subline,
  openGraph: { title: TITLE, description: PRODUCT.subline, url: "/", siteName: PRODUCT.name, type: "website" },
  twitter: { card: "summary", title: TITLE, description: PRODUCT.subline },
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
          {children}
        </ThemeProvider>
      </body>
    </html>
  )
}
