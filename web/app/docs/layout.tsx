import { Ground } from "@/components/ground"

// The docs lie on the page's light alone, never its facets: a page is read at length
// here, and anything moving under the words pulls the eye off them.
export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <Ground facets={false} />
      {children}
    </>
  )
}
