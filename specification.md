# Nikaia Sprachspezifikation
**Version:** 0.0.3 (Experimental Draft)  
**Codename:** Vibe Coding Experiment  
**Datum:** 10. Januar 2026

---

## Kapitel 1: Einleitung und Philosophie

### 1.1. Vorwort: Ein Gedankenexperiment
Nikaia ist in seinem aktuellen Stadium weniger ein fertiges Produkt als vielmehr ein manifestiertes Gedankenexperiment – ein "Vibe Coding Experiment". Es entstand aus einer simplen, aber hartnäckigen Frustration: Der Kluft zwischen dem Code, den wir schreiben wollen (einfach, fließend, den "Happy Path" beschreibend), und dem Code, den wir schreiben müssen, um robuste und performante Systeme zu erhalten (komplex, defensiv, voller technischer Details).

Wir leben in einer Welt, in der Entwickler oft gezwungen sind, sich zu entscheiden: Wähle ich die Entwicklerfreude und Geschwindigkeit von Python oder TypeScript und akzeptiere Laufzeitfehler und Single-Core-Limits? Oder wähle ich die rohe Power und Sicherheit von Rust oder C++ und akzeptiere eine steile Lernkurve und kognitive Dauerbelastung durch Speicherverwaltung?

**Nikaia Version 0.0.3** stellt die These auf, dass dieser Kompromiss nicht zwingend technischer Natur ist, sondern ein Design-Problem der Spracharchitektur. Was wäre, wenn eine Sprache die Ergonomie eines Skripts besitzen könnte, aber unter der Haube, unsichtbar für den Entwickler, die rigorosen Sicherheitsgarantien und die Performance einer Systemsprache erzwingt?

Dies ist keine stabile Software. Es ist eine Einladung an Pioniere, Architekten und Language Engineers, neu darüber nachzudenken, wie sich Systemprogrammierung anfühlen sollte.

### 1.2. Die Vision: Skalierbare Einfachheit
Das Kernziel von Nikaia ist die Vereinigung von Skripting-Ergonomie mit System-Performance.

In herkömmlichen Sprachen diktiert die Syntax oft die Laufzeitumgebung. Wer `await` schreibt, kauft sich eine Event-Loop ein. Wer Threads nutzt, muss Mutexes schreiben. Nikaia bricht mit diesem Dogma. Wir verfolgen einen Ansatz, den wir **"Unified Core Architecture"** nennen.

Der Entwickler schreibt Code in einem linearen, imperativen Stil ("Direct Style"). Der Code beschreibt die Logik, nicht die Mechanik. Es gibt kein sichtbares `await`, keine Callback-Hölle und keine manuellen `Result`-Unwraps, die den Lesefluss stören. Der Nikaia-Compiler agiert als intelligenter Übersetzer, der diese logische Semantik je nach gewünschtem Einsatzprofil in radikal unterschiedliche technische Implementierungen transformiert.

### 1.3. Ein Sprache, zwei Profile
Anstatt eine Sprache zu schaffen, die versucht, "One Size Fits All" zu sein (und dabei oft im Mittelmaß endet), definiert Nikaia zwei explizite Compiler-Profile. Diese Profile teilen sich dieselbe Syntax, denselben Sprachkern und die Dateiendung `.nika`, generieren aber unterschiedliche Laufzeit-Architekturen, um spezifische Engpässe moderner Software zu lösen. Die Unterscheidung erfolgt über die Projektkonfiguration (TOML) oder Compiler-Switches.

#### A. Nikaia Lite (via Switch/Config) – The I/O Engine
Dieses Profil ist unsere Antwort auf Node.js und Go. Es ist optimiert für I/O-Dichte – also Szenarien, in denen ein Programm tausende von Netzwerkverbindungen hält oder Dateien verschiebt, aber pro Verbindung wenig CPU-Last erzeugt.

* **Architektur:** Single-Threaded Async Runtime (basierend auf der Tokio Current-Thread-Runtime).
* **Das Versprechen:** Maximale Kompatibilität bei minimalem Overhead. Da das Programm nur einen einzigen OS-Thread nutzt, entfallen teure Kontextwechsel und die Notwendigkeit für komplexe atomare Synchronisierung.
* **Sicherheit:** In diesem Modus sind Race Conditions (Datenwettläufe) per Design unmöglich. Code zwischen zwei I/O-Punkten läuft garantiert atomar ab. Dies senkt die kognitive Last für den Entwickler massiv.

#### B. Nikaia Standard (via Switch/Config) – The Compute Engine
Dieses Profil ist unsere Antwort auf Rust und C++. Es ist optimiert für CPU-Durchsatz und komplexe Berechnungen.

* **Architektur:** Multi-Threaded Work-Stealing Runtime (Tokio Thread-Pool).
* **Das Versprechen:** Automatische Skalierung über alle verfügbaren Prozessorkerne. Der Compiler injiziert notwendige Synchronisierungs-Primitive und nutzt den Borrow-Checker, um Thread-Sicherheit mathematisch zu beweisen.
* **Realität:** Hier wird Nikaia zu einer echten Systemsprache. Der Entwickler hat Zugriff auf atomare Primitive, parallele `spawn`-Befehle und Low-Level-Optimierungen.

### 1.4. Das Modell der Adaptiven Runtime
Nikaia bricht mit dem Dogma, dass eine Sprache sich für ein technisches Ausführungsmodell entscheiden muss. Anstatt alle Probleme mit demselben Hammer zu bearbeiten, erlaubt Nikaia dem Entwickler, die Laufzeitumgebung (Runtime) an das Last-Profil der Anwendung anzupassen – ohne die Sprache zu wechseln.

Wir nennen dies **"Unified Core, Specialized Runtimes"**.

**Der Migrations-Pfad: Audit statt Rewrite**
Viele Projekte beginnen im Lite-Profil wegen der geringen kognitiven Last. Wenn sich die Anforderungen ändern (z.B. Bedarf an echter Parallelität für Bildverarbeitung), ermöglicht Nikaia einen geführten Übergang.
Der Wechsel vom Lite- zum Standard-Profil ist kein Rewrite, sondern ein Audit. Der Compiler deckt "technische Schulden" (wie unsichere globale Variablen) auf und führt den Entwickler zur Härtung des Systems.

Die primitiven Typen passen sich diesem Wandel polymorph an:
* `spawn`: Delegiert entweder an die lokale Event-Loop (Lite) oder den Thread-Pool (Standard).
* `Locked[T]`: Wird entweder zum schnellen, kooperativen Mutex (Lite) oder zum sicheren OS-Mutex (Standard).

### 1.5. Status des Projekts
Nikaia 0.0.3 ist ein Entwurf. Viele der hier beschriebenen Konzepte sind aktive Forschungsfelder. Wir laden dazu ein, die Grenzen dessen auszuloten, wie modern, sicher und freudvoll Systemprogrammierung sein kann.

---

## Kapitel 2: Der Sprachkern und Nikaia Lite

Dieses Kapitel definiert die fundamentale Syntax, das Typsystem und das Ausführungsmodell von Nikaia 0.0.3. Die hier beschriebenen Konstrukte bilden das gemeinsame Fundament für beide Profile (Lite und Standard).
Das Design folgt dem Prinzip der **"Kognitiven Entlastung"**.

### 2.1. Syntax und Struktur
Nikaia ist eine expressions-orientierte Sprache. Fast jedes Konstrukt (inklusive `if`, `match` und Blöcken) ist ein Ausdruck, der einen Wert zurückgibt.

* **Anweisungen:** Werden primär durch Zeilenumbrüche (Newlines) getrennt.
* **Semikolons (;):** Optional am Zeilenende. Zwingend, wenn mehrere Anweisungen in einer Zeile stehen (`let a = 1; let b = 2`).
* **Blöcke:** `{ ... }` definieren Scopes. Der letzte Ausdruck ist der Rückgabewert.
* **Variablen:**
    * `let x = ...` (Unveränderlich/Immutable).
    * `let mut x = ...` (Veränderlich/Mutable).

### 2.2. Das Datensystem

**Primitive Typen**
* **Integer:** `i32` (Standard), `i64`, `u8`, `isize`.
* **Float:** `f64` (Standard), `f32`.
* **Boolean:** `bool`.
* **Text:** `String` (UTF-8, heap-allokiert) und `&str` (Slice/View).

**Algebraische Datentypen (Enums)**
Enums sind echte Summentypen.

```nika
enum ConnectionState {
    Disconnected,
    Connecting(String),                   // Hält die Ziel-URL
    Connected { ip: String, port: u16 },  // Hält strukturierte Verbindungsdaten
    Error(String)
}
```

**Ergonomische Collections und Proxy-Objekte**
Nikaia bietet Syntax-Zucker für häufige Datenstrukturen.

```nika
// Vektoren
let list = [1, 2, 3]

// Maps & Auto-Vivification mit Entry-Proxies
use std::collections::HashMap

let mut stats = HashMap::with_default(0)

// Index-Zugriff gibt Proxy zurück. 
// `+=` legt Key an (0) falls fehlend, dann addiert 1.
stats["/api/login"] += 1 

// Null Coalescing (??)
let host = env["HOST"] ?? "localhost"
```

### 2.3. Unified Memory Management
Der Compiler substituiert diese Typen basierend auf dem gewählten Profil physikalisch.

* **Shared[T] (Geteilter Besitz):**
    * *Lite:* Kompiliert zu `Rc[T]` (Reference Counted).
    * *Standard:* Kompiliert zu `Arc[T]` (Atomic Reference Counted).
* **Locked[T] (Veränderbarer geteilter Besitz):**
    * *Lite (Cooperative Mutex):* Kompiliert zu einer `RefCell`-Variante (Safe against panics).
    * *Standard (Blocking Mutex):* Kompiliert zu `Mutex[T]`.

**RAII Guards und Scopes**
Der Zugriff erfolgt über `.access()`. Um Deadlocks zu verhindern, erzwingt Nikaia oft explizite Scopes und verbietet implizites Async innerhalb von Locks.

```nika
let state: Shared[Locked[AppConfig]] = ...

state.access() |guard| {
    guard.port = 8080
    // fetch_url() // -> Compiler Error: "Cannot await inside synchronous lock"
}
```

### 2.4. Generics und Traits
Nikaia nutzt eckige Klammern `[...]` für Typ-Parameter.

```nika
// Generische Funktion
fn swap[T](a: &mut T, b: &mut T) {
    let temp = *a; *a = *b; *b = temp
}

// Traits: impl vs with
struct Point with Debug, Clone, PartialEq { x: i32, y: i32 }
```

### 2.5. Fehlerbehandlung ("Direct Style")
Nikaia bricht mit dem Result-Unwrapping zugunsten eines linearen Kontrollflusses.

* **Deklaration:** `throws ErrorType`.
* **Implizite Propagation:** Fehler "bubbeln" automatisch hoch.
* **Handling:** `?{ ... }` Block.

```nika
fn read_config() throws IoError -> String {
    // fs::read wirft IoError -> bubbelt automatisch hoch
    return fs::read_string("config.txt")
}

fn main() {
    let content = read_config()?{
        log_error(error) // `error` ist implizit verfügbar
        return
    }
}
```

**Automatische Kontext-Injektion ("Rich Errors"):**
Der Compiler injiziert bei `throw` automatisch Dateiname, Zeilennummer, Funktionsname und Argumente für "Zero Boilerplate Debugging".

### 2.6. Asynchronität als Standard
Jede Funktion ist potenziell eine asynchrone State-Machine ("Async by Default").

* **Automatic Suspension:** I/O-Operationen (z.B. `socket.read()`) fügen automatisch Suspension-Points ein.
* **Transparenz:** Der Code sieht synchron aus.

```nika
fn download_data() -> String {
    let connection = net::connect("example.com") // suspendiert automatisch
    return connection.read_all()
}
```

**Polymorphe Nebenläufigkeit (`spawn`)**
* **Lite:** Task ist ein "Green Thread" (Concurrency).
* **Standard:** Task läuft im Thread-Pool (Parallelism).

### 2.7. Ressourcen-Management (RAII)
Kein `defer`. Ressourcen implementieren den `Drop`-Trait und werden am Ende des Scopes deterministisch freigegeben.

### 2.8. Module und Einstiegspunkt
Dateibasiertes Modulsystem (`utils.nika` -> `utils`). Einstiegspunkt ist `fn main(args: [String])`.

### 2.9. Typ-Konvertierung
* `as`: Primitive Casts mit potenziellem Datenverlust (`i64` -> `i32`).
* `into()` / `from()`: Semantische Umwandlungen (Move ownership, Zero-Copy).

---

## Kapitel 3: Nikaia Standard – Parallelität und Skalierung

Dieses Profil wird via Config oder `--profile=std` aktiviert. Nikaia wechselt hier von einem kooperativen zu einem präemptiven Modell. Das Design folgt dem Prinzip der **"Inverted Function Coloring"**: Asynchronität ist der Normalfall, Synchronität ist die explizite Einschränkung.

### 3.1. Das Ausführungsmodell: Work Stealing Runtime
* **Thread-Pool:** Worker-Thread pro CPU-Kern.
* **Präemptives Verhalten:** Jederzeit Kontextwechsel möglich.

### 3.2. Asynchronität als Standard ("Async by Default")
Keine Unterscheidung in der Syntax.

**Explizite Parallelität (Fork-Join)**
Das Schlüsselwort `async` dient *ausschließlich* dazu, Parallelität (Forking) anzufordern (deaktiviert das implizite Warten).

```nika
fn main() {
    // 1. Sequentiell (Standard):
    let page1 = fetch_url("a.com") // blockiert logisch

    // 2. Parallel (mit `async` Modifier):
    let future_a = async fetch_url("a.com")
    let future_b = async fetch_url("b.com")

    // 3. Zusammenführung:
    let (p1, p2) = join(future_a, future_b)
}
```

### 3.3. Das "Explicit Sync Constraint" Modell
Funktionen, die **garantiert niemals suspendieren** (kein I/O), müssen explizit mit `sync` markiert werden, um in kritischen Abschnitten genutzt werden zu dürfen.

```nika
// Darf NUR Rechenoperationen nutzen
fn calculate_total(amount: f64) sync -> f64 { ... }

let state: Shared[Locked[OrderState]] = ...
state.access() |guard| {
    // OK: Aufruf von sync-Funktion
    guard.total = calculate_total(100.0)
    
    // FEHLER: `get_tax_rate` ist nicht sync (könnte I/O machen) -> Deadlock-Gefahr
    // let rate = get_tax_rate() 
}
```

**Escape Hatch:** `allow_blocking { ... }` für bewusste Ausnahmen.

### 3.4. Synchronisierung und Speicher
`Locked[T]` wird zu `Mutex[T]` (Blocking OS-Mutex). Daher ist das Verbot von I/O innerhalb von Locks essenziell.

### 3.5. Effect Polymorphismus
Higher-Order-Functions wie `map` inferieren ihren Effekt (Sync vs. Async) basierend auf der übergebenen Closure.

### 3.6. Reaktiver Kontrollfluss: select
Reagiert auf das erste Ereignis und bricht andere Tasks sauber ab (Drop/RAII).

```nika
select {
    val = heavy_computation() => return val,
    _ = sleep(5.seconds()) => throw TimeoutError("Zu langsam!")
}
```

### 3.7. Message Passing (channel)
Typisierte Kanäle als Alternative zu Shared State.
```nika
let (tx, rx) = channel::bounded[String](100)
spawn(move || tx.send("Message"))
```

### 3.8. Low-Level Primitive
Zugriff auf `std::sync::atomic` und `thread_local!`.

### 3.9. Parallelität auf Daten (par_iter)
API für Daten-Parallelität (Work-Stealing). Closures müssen `sync` sein.

```nika
let processed = pixels.par_iter()
    .map(|px| calculate_filter(px)) 
    .collect()
```

### 3.10. Scoped Threads
Ermöglicht "Zero-Copy Parallelism" auf Stack-Daten durch `thread::scope`.

### 3.11. Fault Tolerance: Supervision Trees
Supervised Spawn verlinkt Tasks. Panics werden vom Supervisor abgefangen (Fault Isolation). Strategien: `Restart.OnFail`, `CrashParent`, etc.

---

## Kapitel 4: Metaprogrammierung und Language Engineering

Nikaia betrachtet das Parsen und DSLs als First-Class-Citizens. Philosophie: **"Shift-Left"** – Fehler zur Kompilierzeit erkennen.

### 4.1. Die grammar DSL (Native Parser)
EBNF-Integration im Sprachkern. Kompiliert zu speichersicheren rekursiven Abstiegsparsern in Rust.

```nika
grammar ColorParser {
    option recursion_limit = 50;

    pub rule entry -> Color = {
        "#" r:hex() g:hex() b:hex()
    } -> { Color { r, g, b } }
    
    rule hex -> u8 = s:regex("[0-9A-Fa-f]{2}") -> { u8::from_str_radix(s, 16)? }
}
```
Inkludiert Stack-Overflow-Schutz und optionale Heap-Promotion ("Stacker").

### 4.2. Dual-Mode Nutzung ("Static & Dynamic")
Eine `grammar` generiert zwei Artefakte:

1.  **Static Embedding (`from ... with`):** Parsing zur Build-Zeit. Bei Syntaxfehlern bricht der Build ab. Resultat ist ein binäres Struct (Zero-Startup-Cost).
    ```nika
    const THEME: Theme = from "assets/defaults.conf" with ThemeParser
    ```
2.  **Runtime Parsing (`::parse`):** Nutzung als Bibliotheksfunktion.
    ```nika
    let theme = ThemeParser::parse(user_input)?
    ```

### 4.3. Erweiterbare Makros (dsl)
Einbettung fremder Syntax in Nikaia-Code (ähnlich Rust Proc-Macros, aber einfacher).

```nika
use nikaia_sql::{Database, sql}

fn query(db: Shared[Database], min_age: i32) {
    // Validiert SQL zur Kompilierzeit gegen das Schema
    let users = dsl sql db {
        SELECT name FROM users WHERE age >= min_age
    }
}
```

### 4.4. Pattern Matching auf ASTs
Tiefgreifendes Destructuring (`match`) auf rekursiven Strukturen, inklusive `@` Bindings und Guards.

### 4.5. Hygiene und Isolation
Nikaia erzwingt hygienische Makros. Variablen aus dem äußeren Scope sind nur sichtbar, wenn explizit injiziert.

### 4.6. Fehlertoleranz und Recovery
Parser unterstützen `recover` Punkte, um nach Syntaxfehlern "wieder aufzusetzen" (wichtig für Language Server).

### 4.7. Zero-Copy Parsing
Native Unterstützung für `Slice` (statt `String`), um Allokationen zu vermeiden. Der Compiler garantiert die Lebensdauer via Borrow-Checker.

### 4.8. DSL-Authoring: Grammatiken als Makros
Eine `grammar` kann selbst als DSL-Implementierung dienen, indem sie Nikaia-AST-Knoten zurückgibt.

```nika
#[macro_export]
grammar sql -> nikaia::ast::Expr {
    pub rule query -> nikaia::ast::Expr = { ... } -> {
        quote! { $context.query(...) }
    }
}
```

### 4.9. Compile-Time Reflection (Sandboxed I/O)
Makros dürfen Dateien und Umgebungsvariablen lesen (für Schemas/Configs).
Der Compiler trackt Datei-Abhängigkeiten und invalidiert den Build-Cache automatisch, wenn sich die externe Datei ändert.
```

