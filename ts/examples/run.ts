// Every rule of SPEC.md, run against the implementation: `bun run examples`.
// Each example asserts what it shows; a failure stops with the example's name.

import {
  compose,
  type Infer,
  parseHeader,
  read,
  setCell,
  setType,
  CellsError,
  write,
} from "../src/cells"

let count = 0
function example(name: string, run: () => void) {
  try {
    run()
    count++
  } catch (e) {
    console.error(`✗ ${name}`)
    throw e
  }
}
function equal<T>(got: T, want: T, what: string) {
  const g = JSON.stringify(got)
  const w = JSON.stringify(want)
  if (g !== w) throw new Error(`${what}: got ${g}, want ${w}`)
}
function throws(run: () => unknown, message: string) {
  try {
    run()
  } catch (e) {
    if (!(e instanceof CellsError)) throw e
    if (!e.message.includes(message))
      throw new Error(`threw "${e.message}", want it to say "${message}"`)
    return
  }
  throw new Error(`did not throw "${message}"`)
}

const BUDGET = `---
type Category = travel|stay|food|gear
total = cost * qty                        // computed, never stored
---
item:text!,category:Category,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date
fuel,travel,84.20,1,yes,2026-08-14
groceries,food,96.75,2,no,2026-08-15
`

example("the budget, end to end", () => {
  const { rows, columns, problems } = read(BUDGET)
  equal(problems, [], "no problems")
  equal(
    columns.map((c) => c.name),
    ["item", "category", "cost", "qty", "paid", "on", "total"],
    "columns, the computed one last",
  )
  equal(rows[1], {
    item: "groceries",
    category: "food",
    cost: 96.75,
    qty: 2,
    paid: false,
    on: "2026-08-15",
    total: 193.5,
  }, "a typed row")
  equal(columns[1]?.type.named, "Category", "the named type kept")
  equal(columns[2]?.type.currency, "EUR", "a currency code is a type")
})

example("round trip: the same bytes", () => {
  for (const text of [
    BUDGET,
    'a,b\r\n"x, y",2\r\n',
    "plain,csv\n1,2",
    'q:text\n"needlessly quoted"\n',
  ])
    equal(write(read(text).doc), text, "written as read")
})

example("an edit changes that cell alone", () => {
  const { doc } = read('item:text!,cost:EUR\n"fuel",84.20\ntolls,23.50\n')
  const changed = write(setCell(doc, 1, "cost", "25.00"))
  equal(changed, 'item:text!,cost:EUR\n"fuel",84.20\ntolls,25.00\n', "only the edited cell")
  equal(
    write(setCell(doc, 0, "item", "fuel, diesel")),
    'item:text!,cost:EUR\n"fuel, diesel",84.20\ntolls,23.50\n',
    "quoted when CSV needs it",
  )
})

example("compose: cells back into the file's own frame", () => {
  const { doc } = read("---\r\ntype C = a|b\r\n---\r\nk:C,n\r\na,1\r\n")
  equal(
    compose(doc, ["k:C", "n"], [["b", "2, or so"]]),
    '---\r\ntype C = a|b\r\n---\r\nk:C,n\r\nb,"2, or so"\r\n',
    "prelude and line ending kept, quoted where needed",
  )
})

example("a type rewritten, in shorthand", () => {
  const { doc } = read("n\n1\n2\n")
  const typed = setType(doc, "n", "int>=0")
  equal(write(typed), "n:int>=0\n1\n2\n", "the header cell rewritten")
})

example("shorthand and explicit mean the same", () => {
  const pairs: [string, string][] = [
    ["text!", "text key"],
    ["text?", "text optional"],
    ["travel|stay|food", "one of travel stay food"],
    ["yes/no", "bool yes no"],
    ["EUR>=0", "EUR min 0"],
    ["num<=10", "num max 10"],
    ["0..100", "num between 0 and 100"],
    ["text! optional", "text key?"],
  ]
  for (const [short, long] of pairs)
    equal(
      parseHeader(`c:${short}`).type,
      parseHeader(`c:${long}`).type,
      `${short} = ${long}`,
    )
})

example("choices quoted when they hold reserved characters", () => {
  const { type } = parseHeader("mood:'yay!'|meh|'so-so?'|'it''s fine'")
  equal(type?.choices, ["yay!", "meh", "so-so?", "it's fine"], "the choices")
  equal(parseHeader("a:one of travel 'sd cards'").type?.choices, ["travel", "sd cards"], "explicit")
})

example("a name may hold anything but :", () => {
  equal(parseHeader("wow!:text").name, "wow!", "a ! in a name")
  equal(parseHeader("plain").type, null, "no : is untyped")
})

example("inference for an untyped column", () => {
  const { columns } = read("a,b,c,d,e,f\n1,1.5,yes,2026-08-14,x,0042\n2,2,no,2026-08-15,y,A12\n")
  equal(
    columns.map((c) => [c.type.base, c.type.inferred, c.type.optional]),
    [
      ["int", true, true],
      ["num", true, true],
      ["bool", true, true],
      ["date", true, true],
      ["text", true, true],
      ["text", true, true],
    ],
    "the plainest kind every value fits",
  )
  equal(columns[2]?.type.words, ["yes", "no"], "the file's words")
})

example("bounds, ranges, lengths, formats", () => {
  const { problems, rows } = read(
    "score:0..10,name:text<=3,on:date(DD/MM/YYYY),at:time(HH:mm),pct:pct\n11,abcd,31/02/2026,25:00,12%\n5,abc,14/08/2026,14:30,7\n",
  )
  equal(
    problems.map((p) => [p.row, p.column]),
    [
      [0, "score"],
      [0, "name"],
      [0, "on"],
      [0, "at"],
    ],
    "each broken cell",
  )
  equal(rows[1], { score: 5, name: "abc", on: "2026-08-14", at: "14:30:00", pct: 7 }, "read by format")
  equal(rows[0]?.pct, 12, "12% is 12")
})

example("a broken cell loads and is reported, never coerced", () => {
  const text = "item:text!,cost:num>=0,paid:yes/no\nfuel,84.20,yes\ntolls,8x.50,yes\ncabin,310.00,maybe\nfuel,1,no\n"
  const { rows, problems, doc } = read(text)
  equal(
    problems.map((p) => p.why),
    ['"8x.50" is not a number', '"maybe" is not yes or no', '"fuel" is the key of row 0 too'],
    "the problems, each saying why",
  )
  equal(rows[1]?.cost, null, "its value is null")
  equal(doc.rows[1]?.[1]?.text, "8x.50", "its text is kept")
  equal(write(doc), text, "and written back untouched")
})

example("a blank cell is missing unless ?", () => {
  const { problems, rows } = read("a:int,b:int?\n,\n")
  equal(problems.map((p) => p.why), ['"" is missing'], "a's blank is a problem")
  equal(rows[0], { a: null, b: null }, "both null")
})

example("every grammar error names where", () => {
  throws(() => read("cost:EURO\n1\n"), 'no type or currency "EURO"')
  throws(() => read("a:'open\n1\n"), "a quote is not closed")
  throws(() => read("a:yes|\n1\n"), "a | is followed by no choice")
  throws(() => read("a:int,b\n1,2,3\n"), "row 0: 3 fields, the header has 2")
  throws(() => read("a:travel|stay>=1\nx\n"), "a choice or a boolean takes no bounds")
  throws(() => read("a:int(YYYY)\n1\n"), "only a date or a time takes a format")
  throws(() => read("a:text int\nx\n"), "two types in one cell")
  throws(() => read("---\nnonsense here\n---\na\n1\n"), "prelude line 1")
  throws(() => read("---\n\n// why\nt = a * b\n---\na\n1\n"), 'no column "b"')
  throws(() => read("---\ntype C = a|b\nk:C\na\n"), "opens with --- and never closes")
  throws(() => read('a\n"never closed\n'), "a quoted field is not closed")
  throws(() => parseHeader("a:between 1 2"), "between takes two numbers and and")
})

example("computed columns: blank or broken inputs make blank cells", () => {
  const { rows } = read("---\nper = cost / qty\n---\ncost:num?,qty:int?\n10,0\n,2\n9,3\n")
  equal(rows.map((r) => r.per), [null, null, 3], "÷0 and a blank are blank")
})

// Compile-time: the row type from a header line (checked by `tsc`, `bun run check`).
type Budget = Infer<"item:text!,category:travel|stay|food|gear,cost:EUR>=0,qty:int>=1,paid:yes/no,on:date,note:text?,mood:one of happy sad">
const row: Budget = {
  item: "fuel",
  category: "travel",
  cost: 84.2,
  qty: 1,
  paid: true,
  on: "2026-08-14",
  note: null,
  mood: "happy",
}
// @ts-expect-error: "boat" is not a category
const wrong: Budget = { ...row, category: "boat" }
void wrong

console.log(`✓ ${count} examples`)
