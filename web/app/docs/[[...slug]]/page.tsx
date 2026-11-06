// A page of the docs: the repository's own markdown (lib/docs), drawn with ui's
// markdown, between the sidebar (every document and the specification's sections) and
// the page's table of contents. Every page is prerendered; a path with no page is a 404.

import type { Metadata } from "next"
import { notFound } from "next/navigation"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { Markdown } from "@/components/ui/markdown"
import { TableOfContents } from "@/components/ui/table-of-contents"
import { type DocPage, docNav, docPage, docPages, neighbours, sourceOf } from "@/lib/docs"
import { PRODUCT } from "@/lib/product"

type Params = { slug?: string[] }

const pathOf = (slug: string[] | undefined) => ["/docs", ...(slug ?? [])].join("/")

export const dynamicParams = false

export async function generateStaticParams(): Promise<Params[]> {
  return (await docPages()).map((p) => ({ slug: p.path.split("/").slice(2) }))
}

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const page = await docPage(pathOf((await params).slug))
  if (!page) return {}
  return {
    title: `${page.title} · ${PRODUCT.name} docs`,
    description: page.text.slice(0, 160),
    alternates: { canonical: page.path },
  }
}

export default async function DocsPage({ params }: { params: Promise<Params> }) {
  const page = await docPage(pathOf((await params).slug))
  if (!page) notFound()
  const [nav, { before, after }] = await Promise.all([docNav(), neighbours(page)])
  return (
    <>
      <SiteHeader />
      <div className="mx-auto grid max-w-6xl gap-10 px-6 pt-8 lg:grid-cols-[13rem_minmax(0,1fr)] xl:grid-cols-[13rem_minmax(0,1fr)_12rem]">
        <nav aria-label="docs" className="self-start text-[13px] lg:sticky lg:top-8">
          <ul className="space-y-5">
            {nav.map((doc) => (
              <li key={doc.path}>
                <SideLink href={doc.path} current={page.path} strong>
                  {doc.title}
                </SideLink>
                {doc.pages.length > 0 && (
                  <ul className="mt-2 space-y-1.5 border-l border-foreground/10 pl-3">
                    {doc.pages.map((p) => (
                      <li key={p.path}>
                        <SideLink href={p.path} current={page.path}>
                          {p.title}
                        </SideLink>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </nav>
        {/* A reading measure, about 68 characters a line, and the body a step brighter
            than the markdown's own: text read at length, not a note beside a picture. */}
        <article className="min-w-0 max-w-[68ch]">
          <Markdown tree={page.tree} linkHeadings className="text-foreground/85" />
          <Foot page={page} before={before} after={after} />
        </article>
        <TableOfContents
          headings={page.headings}
          className="hidden self-start xl:sticky xl:top-8 xl:block"
        />
      </div>
      <SiteFooter />
    </>
  )
}

/** A link of the sidebar, lit where the reader is. */
function SideLink({
  href,
  current,
  strong = false,
  children,
}: {
  href: string
  current: string
  strong?: boolean
  children: React.ReactNode
}) {
  const here = href === current
  return (
    <a
      href={href}
      aria-current={here ? "page" : undefined}
      className={`block leading-snug transition-colors hover:text-foreground ${here ? "text-foreground" : strong ? "text-foreground/85" : "text-foreground/60"} ${strong ? "font-medium" : ""}`}
    >
      {children}
    </a>
  )
}

/** Under the page: where its text is written, and the pages either side of it. */
function Foot({ page, before, after }: { page: DocPage; before?: DocPage; after?: DocPage }) {
  return (
    <footer className="mt-16 space-y-8 border-t border-foreground/10 pt-6 text-[13px] text-muted-foreground">
      <p>
        This page is{" "}
        <a className="text-foreground/80 hover:text-foreground" href={sourceOf(page)}>
          {page.file}
        </a>{" "}
        in the repository: the docs are its own files, edited there.
      </p>
      <div className="flex justify-between gap-6">
        {before ? (
          <a className="hover:text-foreground" href={before.path}>
            ← {before.title}
          </a>
        ) : (
          <span />
        )}
        {after && (
          <a className="text-right hover:text-foreground" href={after.path}>
            {after.title} →
          </a>
        )}
      </div>
    </footer>
  )
}
