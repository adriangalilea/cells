import type { Metadata, Viewport } from "next"
import { Geist, Geist_Mono } from "next/font/google"
import { cardImage } from "@/lib/card"
import { PRODUCT } from "@/lib/product"
import "./globals.css"

const geist = Geist({ subsets: ["latin"], variable: "--font-geist" })
const mono = Geist_Mono({ subsets: ["latin"], variable: "--font-geist-mono" })

const TITLE = `${PRODUCT.name}: ${PRODUCT.tagline.replace(/\.$/, "")}`
/** The link preview's picture (lib/card, drawn by scripts/og.ts). */
const CARD = cardImage()

export const metadata: Metadata = {
  metadataBase: new URL(`https://${PRODUCT.domain}`),
  title: TITLE,
  description: PRODUCT.subline,
  openGraph: { title: TITLE, description: PRODUCT.subline, url: "/", siteName: PRODUCT.name, type: "website", images: [CARD] },
  twitter: { card: "summary_large_image", title: TITLE, description: PRODUCT.subline, images: [CARD] },
}

// The site is dark only: the class and the colour scheme are in the HTML, so the first
// paint is the theme with no script to set it.
export const viewport: Viewport = { themeColor: "#0b0d11", colorScheme: "dark" }

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`dark ${geist.variable} ${mono.variable}`} style={{ colorScheme: "dark" }}>
      <body>{children}</body>
    </html>
  )
}
