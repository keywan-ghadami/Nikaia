# Branch-Review: `0.0.5`, `v0.0.6` und `claude/nikaia-language-concepts-uus8q5`

**Datum:** 5. September 2026
**Anlass:** Nach dem Aufräumen in `claude/nikaia-language-concepts-uus8q5` prüfen, was in den
Branches `0.0.5` und `v0.0.6` übersehen wurde.

---

## 1. Kernbefund (TL;DR)

1. **Baut `v0.0.6` auf `0.0.5` auf?** — **Inhaltlich ja, git-technisch nein.** `v0.0.6` zweigt von
   einem `main`-Commit ab, der den gesquashten `0.0.5`-Stand bereits enthält. Der RFC 0.0.6
   deprecatet `0.0.5` explizit in einem eigenen Kapitel („Analysis of Status Quo").
2. **Beide Branches wurden vollständig gemerged** — per *Squash*-Merge, deshalb sind die
   Branch-Spitzen keine Vorfahren von `main` und wirken auf GitHub „offen".
   Inhaltlich verifiziert: Merge-Trees sind identisch mit den Branch-Spitzen. Es ging beim
   Merge nichts verloren.
3. **Aber: Der komplette `v0.0.6`-Inhalt wurde später auf `main` wieder entfernt.**
   Commit `c6ff1fa` („update spec", 19.02.2026 — der letzte Commit auf `main`) setzt die drei
   Spec-Dateien byte-genau auf den Stand *vor* PR #2 zurück. Zusammen mit `9fcf21c`
   („delete rfc because it was done", 25.01.) ist die 0.0.6-Architektur heute **nirgends mehr
   in einem lebenden Branch vorhanden** — nur noch in der Git-History und in `origin/v0.0.6`.
4. **Folgefehler:** Der Concepts-Branch hat auf diesem zurückgesetzten Stand aufgesetzt und die
   Spec auf „0.0.6" hochgezogen. Es gibt jetzt **zwei verschiedene, inkompatible 0.0.6**.

---

## 2. Topologie (verifiziert)

```
713b9fe (18.01.) ─┬─────────────────────────────────────────────── main
                  │                                                 │
                  └── 0.0.5 (16 Commits) ──┐                        │
                                           └─ squash → a02b164 (#1, 23.01.)
                                                        │
                                            50a48c8/66034f7 (24.01.) RFC 0.0.6 direkt auf main
                                                        │
                              ┌─────────────────────────┤
                              │                         │
                    v0.0.6 (3 Commits) ──┐              │
                                         └─ squash → c0891c0 (#2, 25.01.)
                                                        │
                                            9fcf21c (25.01.) RFC-Datei gelöscht
                                                        │
                                            … 30 Commits Toolchain/Parser …
                                                        │
                                            c6ff1fa (19.02.) ⚠ REVERT der v0.0.6-Spec
                                                        │
                              claude/nikaia-language-concepts-uus8q5 (4 Commits, 05.09.)
```

**Beweise:**

| Prüfung | Ergebnis |
| :--- | :--- |
| `merge-base --is-ancestor origin/0.0.5 origin/v0.0.6` | nein (Squash-Merge) |
| `diff a02b164 origin/0.0.5 -- docs/` | leer → PR #1 hat 0.0.5 vollständig übernommen |
| `diff c0891c0 origin/v0.0.6 -- docs/` | leer → PR #2 hat v0.0.6 vollständig übernommen |
| `diff e915917 c0891c0 -- <3 Spec-Dateien>` | leer → v0.0.6-Inhalt war bis kurz vor `c6ff1fa` intakt |
| `diff origin/main 66034f7 -- <3 Spec-Dateien>` | leer → `main` = Stand **vor** v0.0.6 |

Das dritte `v0.0.6`-Commit (`9d2b382`, „inline assembly spec to use immediate capture") ist im
Merge-Body von PR #2 nicht aufgeführt, im Merge-Tree aber enthalten — es ist also mitgemerged
und mit `c6ff1fa` mitverloren gegangen.

---

## 3. Was wir übersehen haben: die verlorenen Ideen aus `v0.0.6`

Alle folgenden Punkte stehen heute **weder in `main` noch im Concepts-Branch**.

### 3.1. Scannerless Grammar Protocol (kein globaler Lexer)

Der Compiler-Kern gibt den rohen Byte-Cursor an die DSL-Grammatik ab, statt einen globalen
Token-Stream zu erzeugen.

> **Begründung (RFC §2.2):** Ein globaler Lexer kann eingebettete Sprachen mit
> konfligierenden Delimitern nicht korrekt tokenisieren (JSON `}` vs. Block-Ende,
> Rust-Lifetimes `'a` vs. Char-Literal).

Das ist eine **Architekturentscheidung am Compiler-Kern**, keine Syntax-Kosmetik — und sie
betrifft direkt die aktuelle Parser-Arbeit auf `main` (winnow-grammar).

### 3.2. `} eod` — explizites DSL-Ende für paralleles Parsen

```nika
let query = dsl mysql {
    SELECT name FROM users WHERE age >= :target_age
} eod
```

Der Kern-Parser scannt vorab nur bis `} eod`, überspringt den Body und kann **den Rest der Datei
parallel weiterparsen**, ohne auf den DSL-Parser zu warten. Ein Performance-Argument, das ohne
den Marker nicht funktioniert.

### 3.3. Grammar-Syntax 2.0: Commit Points, Auto-AST, Regex-Terminals

* **Commit Points (`=>`):** steuert Backtracking. Nach dem Commit-Punkt gibt es keine
  Alternative mehr, sondern einen Fehler — die Voraussetzung für brauchbare Parser-Fehlermeldungen.
* **Auto-AST:** ohne explizites `-> {}` generiert Nikaia Structs (labeled sequences) und Enums
  (Alternativen) automatisch.
* **Regex-Terminals:** `regex("...")` kompiliert zu DFAs auf dem Byte-Stream.

Auch in `10.7 Fault Tolerance` schlug sich das nieder: `"{" => statements:stmt()* "}"`.

### 3.4. DSL als Ausdruck — eine Form statt zwei

| 0.0.5 | v0.0.6 |
| :--- | :--- |
| `const T: Color = from "f.conf" with ColorParser` | `const C: Json::Value = dsl Json from "config.json"` |
| `ColorParser::parse(input)?` | `dsl Json from input` |

Compile-Time- und Runtime-Parsing bekommen **dieselbe Syntax**. Das ist die konsequente
Weiterführung des „Dual-Mode Parsing"-Versprechens aus 0.0.5, das dort noch zwei Mechanismen
brauchte.

### 3.5. Hybrid Binding — die eigentliche Erkenntnis des RFC

> **RFC §2.3:** Reines Scope-Binding ist für Assembly perfekt, für SQL Prepared Statements aber
> **fatal** — es backt die Variablenwerte in die Statement-Definition ein und verhindert
> Wiederverwendung.

Daraus zwei Primitive:

* `meta::capture(id)` — **Immediate Capture**: löst zur Compile-Zeit aus dem umgebenden Scope auf (`$table`, ASM-Operanden).
* `meta::parameter(name, type)` — **Deferred Parameter**: benannter Laufzeitparameter (`:id`).

### 3.6. Shadow Types + Typed Spread (`...args: Self::dsl`)

Enthält ein DSL-Block `meta::parameter`, erzeugt der Compiler einen **Shadow Type**, der die
Signatur der gefundenen Parameter trägt: `SqlAst<Signature={id: i32, active: bool}>`.

```nika
impl SqlParser {
    pub fn execute(self; ...args: Self::dsl) -> Result<Row> { … }
}

stm.execute(; target_id: 501)   // Compiler erzwingt alle im DSL gefundenen Parameter
```

DSL-Parameter sind per Definition **Config** (benannt, hinter dem `;`) — das ist die
Anwendung des Subject-;-Config-Protokolls aus 0.0.5 auf Metaprogrammierung.
Monomorphisiert, Stack-Allokation, Zero-Allocation.

### 3.7. `unsafe asm` raus aus dem Sprachkern (WASM-Kompatibilität)

Der härteste verlorene Punkt. `v0.0.6` **löscht** das komplette Kapitel 16 mit `unsafe asm`,
der `reg`/`freg`/`mem`/`imm`/`reg_or_mem`-Constraint-Tabelle und `clobber("cc")` und ersetzt es
durch bibliotheksdefinierte DSLs:

```nika
use std::backend::x86

dsl x86 {
    $v = in(reg) val
    $r = out(reg) result
    mov $r, $v
} eod
```

> **RFC §2.1:** Register-Constraints im Kern koppeln die Sprache an registerbasierte CPUs und
> machen sie **inkompatibel zu Stack-Maschinen wie WebAssembly**.

**`main` enthält heute wieder das alte `unsafe asm`-Kapitel (Zeilen 251–330 in
`30-nikaia-tooling.md`).** Diese Architekturentscheidung ist stillschweigend rückgängig gemacht.

### 3.8. Kleinere Verluste

* `dsl js` mit Parameter-Löchern (`:msg` + `script.exec(; msg: message)`) statt
  String-Interpolation `{message}` — `main` hat wieder die Interpolationsvariante.
* `dsl sql db { … } eod` in Kapitel 17 — `main` hat wieder die Form ohne `eod` (Zeile 390).
* RFC §6 „Implementation Strategy": Monomorphisierung pro DSL-String,
  **Context-Aware SIMD** (Parser nutzen SIMD-Instruktionen, die auf den konkreten
  eingebetteten Syntax-Kontext optimiert sind).

---

## 4. Aus `0.0.5`: nichts verloren

Die drei Spec-Dateien auf `main` sind **byte-identisch** mit der `0.0.5`-Branchspitze.
Alles aus 0.0.5 ist da: Nullable Types (`String?`, `?.`, `??`), Private-by-Default +
anonymer Konstruktor, das **Subject `;` Config**-Protokoll, `fn:`/`fn { }`-Lambdas,
Contextual Capture (`@immediate`/`@detached`), `catch` statt `?{ }`, Task Poisoning,
Appendix B (Capture-Attribute).

**Ein Nebenbefund:** `adr-001.md` wurde nach dem 0.0.5-Merge auf `syn-grammar` umgeschrieben
(Zeile 79 ff.), `Cargo.toml` auf `main` nutzt aber inzwischen **`winnow-grammar`**.
ADR-001 ist gegenüber der tatsächlichen Toolchain veraltet.

---

## 5. Konvergenz: gleiche Schlüsse auf verschiedenen Wegen

### 5.1. Zero-Copy Parsing (10.6) — **beide Branches haben genau diesen Abschnitt umgeschrieben**

| | `v0.0.6` | Concepts-Branch |
| :--- | :--- | :--- |
| Titel | „Zero-Copy Parsing (**Scannerless**)" | „Zero-Copy Parsing (**Tethered Slices**)" |
| Mechanismus | Slices in den Byte-Stream, kein Token-Stream | `Shared`-Handle auf den Buffer + Offset (`bytes::Bytes`-Modell) |
| Lebenszeit-Argument | „der Borrow-Checker stellt sicher, dass du ein Token nicht nach Löschen des Textes nutzt" | „der Quelltext **kann nicht** freigegeben werden, solange ein Token hineinzeigt" |

Das ist der interessanteste Punkt: **beide sahen dasselbe Problem, aber nur der Concepts-Branch
hat es gelöst.** ADR-005 zeigt, dass die v0.0.6-Formulierung falsch ist — der Borrow-Checker
würde gespeicherte Tokens *ablehnen*, nicht absichern. Die Tethered-Slices-Antwort ist die
richtige.

Umgekehrt fehlt dem Concepts-Branch die *scannerless*-Rahmung (Slices in einen **Byte-Stream**
statt in einen Token-Stream). Beide Texte gehören zusammengeführt, nicht gegeneinander gestellt.
Auch RFC §6 („direct byte-stream consumers", Context-Aware SIMD) passt genau in die
Optimierungs-Fußnote von ADR-005 D4.

### 5.2. Immediate vs. Deferred — dreimal unabhängig gefunden

Der stärkste Konvergenzbefund. Dieselbe Achse taucht in drei getrennten Arbeitssträngen auf:

* **0.0.5, Kap. 5.4:** `@immediate` (Borrow) vs. `@detached` (Move) für Lambdas.
* **v0.0.6, RFC §5.1:** `meta::capture` (immediate, Compile-Zeit) vs. `meta::parameter`
  (deferred, Laufzeit) für DSL-Bindings.
* **Concepts, ADR-006 / 12.7:** `task::scope` als Immediate Context vs. *parked cleanup*
  als deferred Teardown.

Das ist kein Zufall, sondern ein durchgehendes Sprachprinzip, das bisher nirgends als solches
benannt ist. Es gehört als Querschnittsprinzip in die Spec.

### 5.3. Compile-Zeit-Garantie statt Laufzeit-Check

Gleicher Instinkt, verschiedene Domänen:

* **v0.0.6:** Commit Points erzwingen Fehler statt stillem Backtracking; SQL-/ASM-Operanden
  werden zur Compile-Zeit validiert.
* **Concepts:** `NK2201` (Lock verlangt `sync`-Lambda) macht aus „schlaf nicht im Lock" eine
  Garantie; `NK2102` für scoped Tasks. Runtime-Checks werden explizit zum *Backstop* degradiert.

Kein Widerspruch — dieselbe Philosophie, die sich gegenseitig stützt.

### 5.4. WASM / Profil-Portabilität als Architekturdruck

* **v0.0.6 §2.1:** Assembly muss aus dem Kern, weil WASM eine Stack-Maschine ist.
* **Concepts ADR-006:** ehrliche Lite/WASM-Grenzen — Panic-Hook vor dem WASM-Trap,
  `panic = abort`, blockierendes FFI in Lite.

v0.0.6 beschreibt die **Ursache**, der Concepts-Branch die **Konsequenzen**. Komplementär.

### 5.5. Subject `;` Config

0.0.5 führt es ein, v0.0.6 dehnt es auf DSL-Parameter aus (`stm.execute(; target_id: 501)`),
der Concepts-Branch verwendet es durchgängig, fasst die DSL-Seite aber nie an.
Reine Erweiterung, kein Konflikt.

---

## 6. Konflikte und offene Punkte

| # | Problem | Schwere |
| :-- | :--- | :--- |
| K1 | **Zwei verschiedene „0.0.6"** — PR #2 (Grammar-Protokoll) und Concepts-Branch (Borrow/Cleanup) tragen dieselbe Nummer bei unterschiedlichem Inhalt. | hoch |
| K2 | **Kapitel 16** — `main` hat `unsafe asm` im Kern zurück; v0.0.6 hatte es bewusst entfernt (WASM). Entscheidung muss neu getroffen und dokumentiert werden. | hoch |
| K3 | **10.6 textueller Merge-Konflikt** — beide Branches haben denselben Abschnitt neu geschrieben. | mittel |
| K4 | **RFC 0.0.6 ist gelöscht** — die Begründungen („warum scannerless", „warum Hybrid Binding") existieren nur noch in der History. Das Repo nutzt inzwischen ADRs; ein ADR-007 wäre der richtige Ort. | mittel |
| K5 | **`dsl sql db { }` ohne `eod`** (Kap. 17, Zeile 390) und `dsl js` mit `{message}`-Interpolation stehen wieder auf `main`. | niedrig |
| K6 | **ADR-001 veraltet** — beschreibt `syn-grammar`, `Cargo.toml` nutzt `winnow-grammar`. | niedrig |

---

## 7. Empfehlung

1. **K1 klären, bevor irgendwas gemerged wird.** Entweder wird die Ziel-Spec-Version 0.0.6 als
   *Vereinigung* beider Stränge definiert, oder die Concepts-Arbeit wird 0.0.7 und v0.0.6 wird
   zuerst wiederhergestellt.
2. **`c6ff1fa` gezielt rückgängig machen** (nur die drei Spec-Dateien), dann den Concepts-Branch
   darauf rebasen. So kommen die verlorenen Kapitel 10.1/10.2/10.5/16 zurück.
3. **10.6 manuell zusammenführen:** Tethered-Slices-Semantik (Concepts) + scannerless
   Byte-Stream-Rahmung (v0.0.6).
4. **ADR-007 „Scannerless Grammar Protocol & Hybrid Binding"** aus dem RFC-Text schreiben —
   inklusive der drei Ablehnungsgründe gegen 0.0.5 (Register-Kopplung, Lexer-Grenzen,
   Stale Closures) und der verworfenen Alternativen.
5. **Immediate/Deferred als Querschnittsprinzip benennen** (Abschnitt 5.2) und aus Kap. 5.4
   heraus auf Bindings und Cleanup verlinken.
6. **ADR-001 auf `winnow-grammar` aktualisieren.**
