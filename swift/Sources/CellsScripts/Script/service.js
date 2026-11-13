// A Cells script's TypeScript language service, one file, in memory. Loaded after
// TypeScript's own typescript.js into the same context. `cellsLibs` is set by the host:
// every lib file's name to its text, the ambient script context among them.
//
// The script is `export default async function ({ last }) { … }`: a declaration, whose
// parameter nothing types. The service reads a shadow of it in which the function is an
// expression satisfying `CellsScript`, so its parameter is typed by context, and maps
// every offset back to the code as written.

var cellsService = (function () {
    var file = "script.ts";
    var row = "cells-row.d.ts";
    var shadow = "";
    var version = 0;
    var rowText = "interface CellsRow {}\n";
    var rowVersion = 0;
    var options = {
        target: ts.ScriptTarget.ES2022,
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        lib: ["lib.es2022.d.ts"],
        types: [],
        strict: true,
        noEmit: true,
        allowJs: false,
        skipLibCheck: true,
    };
    var snapshots = {};
    function snapshot(name, text) {
        var s = snapshots[name];
        if (!s) {
            s = ts.ScriptSnapshot.fromString(text);
            snapshots[name] = s;
        }
        return s;
    }
    var host = {
        getCompilationSettings: function () { return options; },
        getScriptFileNames: function () { return [file, "cells-script.d.ts", row]; },
        getScriptVersion: function (name) {
            if (name === file) return String(version);
            if (name === row) return String(rowVersion);
            return "0";
        },
        getScriptSnapshot: function (name) {
            if (name === file) return ts.ScriptSnapshot.fromString(shadow);
            if (name === row) return ts.ScriptSnapshot.fromString(rowText);
            var text = cellsLibs[name];
            return text === undefined ? undefined : snapshot(name, text);
        },
        getCurrentDirectory: function () { return ""; },
        getDefaultLibFileName: function () { return "lib.es2022.d.ts"; },
        fileExists: function (name) {
            return name === file || name === row || cellsLibs[name] !== undefined;
        },
        readFile: function (name) {
            if (name === file) return shadow;
            if (name === row) return rowText;
            return cellsLibs[name];
        },
        useCaseSensitiveFileNames: function () { return true; },
    };
    var service = ts.createLanguageService(host, ts.createDocumentRegistry());

    // The shadow: `(` before the default export's function, `) satisfies CellsScript`
    // after it. Returns the offsets inserted, in order, to map positions both ways.
    function shade(code) {
        var source = ts.createSourceFile(file, code, ts.ScriptTarget.ES2022, true);
        var inserts = [];
        for (var i = 0; i < source.statements.length; i++) {
            var s = source.statements[i];
            if (!ts.isFunctionDeclaration(s) || s.name) continue;
            var mods = ts.getModifiers(s) || [];
            var exported = mods.some(function (m) { return m.kind === ts.SyntaxKind.ExportKeyword; });
            var deflt = mods.some(function (m) { return m.kind === ts.SyntaxKind.DefaultKeyword; });
            if (!exported || !deflt) continue;
            // the function starts after `export default `: at `async`, else `function`
            var start = -1;
            for (var j = 0; j < mods.length; j++) {
                if (mods[j].kind === ts.SyntaxKind.AsyncKeyword) start = mods[j].getStart(source);
            }
            if (start < 0) {
                start = s.getStart(source);
                var text = code.slice(start, s.end);
                var at = text.search(/\bfunction\b/);
                if (at < 0) throw new Error("cells: a default export with no function keyword");
                start += at;
            }
            inserts.push({ at: start, text: "(" });
            inserts.push({ at: s.end, text: ") satisfies CellsScript" });
            break;
        }
        var out = "";
        var from = 0;
        for (var k = 0; k < inserts.length; k++) {
            out += code.slice(from, inserts[k].at) + inserts[k].text;
            from = inserts[k].at;
        }
        out += code.slice(from);
        return { text: out, inserts: inserts };
    }

    function toShadow(inserts, offset) {
        var shifted = offset;
        for (var i = 0; i < inserts.length; i++) {
            if (inserts[i].at < offset || (inserts[i].at === offset && i === 0)) shifted += inserts[i].text.length;
        }
        return shifted;
    }

    function fromShadow(inserts, offset) {
        var back = offset;
        var acc = 0;
        for (var i = 0; i < inserts.length; i++) {
            var at = inserts[i].at + acc;
            if (offset > at) back -= Math.min(inserts[i].text.length, offset - at);
            acc += inserts[i].text.length;
        }
        return back;
    }

    // The script and its table's columns handed to the service: `columns` the column
    // names, JSON, each a string field of a row. Returns the shadow's inserts.
    function prepare(code, columns) {
        var names = JSON.parse(columns);
        // a column named as an identifier is written bare, so TypeScript says `CellsRow.date`;
        // any other name quoted
        var text = "interface CellsRow {\n" + names.map(function (n) {
            var key = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(n) ? n : JSON.stringify(n);
            return "    " + key + ": string;\n";
        }).join("") + "}\n";
        if (text !== rowText) {
            rowText = text;
            rowVersion += 1;
        }
        var shaded = shade(code);
        if (shaded.text !== shadow) {
            shadow = shaded.text;
            version += 1;
        }
        return shaded.inserts;
    }

    // Parts TypeScript says (a signature, documentation) as one text.
    function said(parts) {
        return parts ? ts.displayPartsToString(parts) : "";
    }

    return {
        // What a completion named `name` at `offset` is: its signature, then what its
        // documentation says. "" where TypeScript says nothing.
        details: function (code, offset, columns, name) {
            var inserts = prepare(code, columns);
            var d = service.getCompletionEntryDetails(file, toShadow(inserts, offset), name, {}, undefined, {}, undefined);
            if (!d) return "";
            var doc = said(d.documentation);
            return said(d.displayParts) + (doc ? "\n" + doc : "");
        },
        // What is at `offset`: [text, from, to], JSON, or "null".
        hover: function (code, offset, columns) {
            var inserts = prepare(code, columns);
            var q = service.getQuickInfoAtPosition(file, toShadow(inserts, offset));
            if (!q) return "null";
            var doc = said(q.documentation);
            var text = said(q.displayParts) + (doc ? "\n" + doc : "");
            var from = fromShadow(inserts, q.textSpan.start);
            var to = fromShadow(inserts, q.textSpan.start + q.textSpan.length);
            return JSON.stringify([text, from, to]);
        },
        complete: function (code, offset, columns) {
            var inserts = prepare(code, columns);
            var shaded = { inserts: inserts };
            var at = toShadow(shaded.inserts, offset);
            var result = service.getCompletionsAtPosition(file, at, {
                includeCompletionsWithInsertText: true,
                includeCompletionsForModuleExports: false,
            });
            if (!result) return "[]";
            var span = result.optionalReplacementSpan;
            var items = result.entries.map(function (e) {
                var r = e.replacementSpan || span;
                var from = r ? fromShadow(shaded.inserts, r.start) : offset;
                var to = r ? fromShadow(shaded.inserts, r.start + r.length) : offset;
                return [e.name, e.kind, e.kindModifiers || "", e.sortText, from, to, e.insertText || ""];
            });
            return JSON.stringify(items);
        },
    };
})();
