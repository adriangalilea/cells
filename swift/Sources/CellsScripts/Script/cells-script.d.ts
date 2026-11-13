// What a Cells script sees: Bun's web-standard globals it uses, and its argument.

interface Headers {
    get(name: string): string | null;
    has(name: string): boolean;
    set(name: string, value: string): void;
    forEach(fn: (value: string, key: string) => void): void;
}
declare var Headers: { new (init?: Record<string, string> | [string, string][]): Headers };

interface Body {
    readonly body: unknown;
    readonly bodyUsed: boolean;
    arrayBuffer(): Promise<ArrayBuffer>;
    json(): Promise<any>;
    text(): Promise<string>;
}

interface RequestInit {
    method?: string;
    headers?: Record<string, string> | Headers;
    body?: string | ArrayBuffer | Uint8Array;
    signal?: AbortSignal;
}

interface Response extends Body {
    readonly headers: Headers;
    readonly ok: boolean;
    readonly redirected: boolean;
    readonly status: number;
    readonly statusText: string;
    readonly url: string;
    clone(): Response;
}
declare var Response: { new (body?: string | ArrayBuffer | null, init?: { status?: number; headers?: Record<string, string> }): Response; json(data: unknown): Response };

interface AbortSignal { readonly aborted: boolean }
declare var AbortSignal: { timeout(ms: number): AbortSignal };

declare function fetch(input: string | URL, init?: RequestInit): Promise<Response>;

interface URLSearchParams {
    append(name: string, value: string): void;
    get(name: string): string | null;
    set(name: string, value: string): void;
    toString(): string;
}
declare var URLSearchParams: { new (init?: string | Record<string, string>): URLSearchParams };

interface URL {
    hash: string;
    host: string;
    hostname: string;
    href: string;
    readonly origin: string;
    pathname: string;
    search: string;
    readonly searchParams: URLSearchParams;
    toString(): string;
}
declare var URL: { new (url: string, base?: string | URL): URL };

interface Console {
    log(...data: any[]): void;
    warn(...data: any[]): void;
    error(...data: any[]): void;
}
declare var console: Console;

// `CellsRow`, a row of the script's table by its columns, is declared per file by the
// service from the table's fence (cells-row.d.ts).

/** What a script is called with. */
interface CellsRun {
    /** The table's last row as the file keeps it, its columns by name. */
    readonly last: CellsRow;
    /** Every row the file keeps. */
    readonly rows: readonly CellsRow[];
}

/** A script: called with the table it fills, it gives the rows to keep. */
type CellsScript = (run: CellsRun) => Promise<CellsRow[]> | CellsRow[];
