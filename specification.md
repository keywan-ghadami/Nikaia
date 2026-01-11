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
Eine mit sync markierte Funktion darf ausschließlich andere sync-Funktionen oder primitive Operationen aufrufen. Der Aufruf einer Standard-Funktion (implizit async) ist ein Kompilierfehler.

Closures innerhalb einer sync-Funktion erben automatisch den sync-Constraint ("Contextual Inference").

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

## Kapitel 5: Epilog und Ausblick

### 5.1. Zusammenfassung
Nikaia 0.0.3 ist der Versuch, die Grenzen der Systemprogrammierung neu zu ziehen. Wir haben in dieser Spezifikation gezeigt, dass die traditionellen Kompromisse der Softwareentwicklung durch Architektur gelöst werden können:
* **Performance** erfordert keine manuelle Speicherverwaltung, wenn der Compiler klug genug ist, primitive Typen je nach Profil auszutauschen (Unified Types).
* **Asynchronität** erfordert keine "eingefärbten Funktionen" (Function Coloring) oder komplexe Syntax, wenn die Runtime adaptiv ist und der Compiler State-Machines im Hintergrund generiert.
* **Sicherheit** muss nicht auf Kosten der Entwicklerfreude gehen ("Vibe Coding"), wenn man einen linearen "Direct Style" mit strengen, unsichtbaren Compiler-Checks kombiniert.

### 5.2. Der Weg zu 0.1.0
Dieses Dokument ist ein "Experimental Draft". Um Nikaia von einem Gedankenexperiment in eine nutzbare Sprache zu verwandeln, sind dies die nächsten logischen Schritte:
* **Bootstrap:** Implementierung eines minimalen Parsers in Rust, der Nikaia-Lite Code in Rust-Code transpiliert.
* **Runtime-Integration:** Verheiratung der Tokio-Runtime (sowohl Current-Thread als auch Thread-Pool) mit dem generierten Code.
* **Standard Library:** Definition der Kern-Module (`std::fs`, `std::net`, `std::sync`) basierend auf den Unified Types `Shared` und `Locked`.

---

## Kapitel 6: Ökosystem, Interoperabilität und Tooling

Damit Nikaia von einem theoretischen Konstrukt zu einer produktiven Umgebung wird, definiert dieses Kapitel die Schnittstellen zur Außenwelt, die Organisation von Code und die Mechanismen zur Qualitätssicherung.

### 6.1. Benutzerdefinierte Traits (Interfaces)
Während Kapitel 2.4 die Nutzung von Traits (`impl`, `with`) beschrieb, ist die Definition eigener Verhaltensverträge essenziell für generische Programmierung.

* **Definition:** Traits definieren Methoden-Signaturen. Sie können Default-Implementierungen enthalten.
* **Associated Types:** Nikaia verzichtet in v0.0.3 auf komplexe Associated Types zugunsten von Generics, um die Typ-Inferenz schnell zu halten.

```nika
// Definition eines Verhaltens
trait Drawable {
    // Erforderliche Methode
    fn draw(&self) -> String
    
    // Optionale Methode mit Default-Logik
    fn is_visible(&self) -> bool { true }
}

// Implementierung
struct Button { label: String }

impl Drawable for Button {
    fn draw(&self) -> String { "[Button: {self.label}]" }
}

// Nutzung als Bound (Einschränkung)
fn render_screen[T: Drawable](items: [T]) {
    for item in items {
        if item.is_visible() { println(item.draw()) }
    }
}
```

### 6.2. Test-First Architektur
Testing ist in Nikaia kein externer Prozess, sondern Teil der Sprachgrammatik. Dies fördert eine Kultur, in der Tests parallel zum Code entstehen.

* **Test-Blöcke:** Der `test`-Block ist ein Top-Level-Konstrukt.
* **Kompilierung:** Diese Blöcke werden nur kompiliert, wenn der Compiler im Test-Modus (`nikaia test`) läuft. Im Release-Build werden sie restlos entfernt.
* **Sichtbarkeit:** Tests haben Zugriff auf private Felder des Moduls, in dem sie definiert sind.

```nika
fn add(a: i32, b: i32) -> i32 { a + b }

// Unit Test
test "Addition funktioniert" {
    assert add(2, 2) == 4
    assert add(-1, 1) == 0
}

// Property-Based Testing (Fuzzing Integration)
// Der Compiler generiert automatisch Zufallswerte für typisierte Argumente
test "Kommutativität" (a: i32, b: i32) {
    assert add(a, b) == add(b, a)
}
```

### 6.3. Globale Zustände (Statics)
Der Umgang mit globalen Variablen ist der kritischste Punkt im Dual-Profile-Design (Lite vs. Standard).

* **Problem:** Eine globale Variable, die im Lite-Profil (Single Threaded) sicher ist, verursacht im Standard-Profil (Multi Threaded) sofort Data Races.
* **Lösung:** Nikaia verbietet unsichere globale Mutation (`static mut`). Globale Variablen müssen entweder unveränderlich sein oder Thread-Safe-Container nutzen.

```nika
// Erlaubt: Konstante Daten (werden ins Read-Only Segment kompiliert)
static APP_VERSION: &str = "1.0.0"

// Erlaubt: Veränderlicher globaler Zustand
// Muss in `Locked` verpackt sein.
// - Lite Profil: Kompiliert zu RefCell (Zero Overhead)
// - Standard Profil: Kompiliert zu Mutex (Thread Safe)
static GLOBAL_CONFIG: Locked[Config] = Locked::new(Config::default())

fn update_config() {
    GLOBAL_CONFIG.access() |cfg| {
        cfg.debug_mode = true
    }
}
```

### 6.4. Interoperabilität (FFI und Unsafe)
Nikaia ist pragmatisch. Um System-APIs zu nutzen oder mit C-Bibliotheken zu interagieren, bietet es kontrollierte Auswege aus dem Sicherheitskäfig.

* `extern` Blöcke: Importieren von C-Symbolen.
* `unsafe` Blöcke: Markieren Code-Abschnitte, in denen der Borrow-Checker und Typ-Sicherheitsprüfungen ausgesetzt sind. Hier liegt die Verantwortung für Speicherzugriffe beim Entwickler.

```nika
// Import der C-Standardbibliothek
extern "C" {
    fn malloc(size: usize) -> Pointer[u8]
    fn free(ptr: Pointer[u8])
}

fn raw_memory_op() {
    unsafe {
        let ptr = malloc(1024)
        // Direkter Pointer-Zugriff ohne Bounds-Check
        ptr.write(0, 255) 
        free(ptr)
    }
}
```

### 6.5. Projekt-Konfiguration (Das Manifest)
Nikaia-Projekte werden durch eine `nikaia.toml` definiert. Hier wird auch das Standard-Profil für das Projekt festgelegt.

```toml
[package]
name = "hyper-core"
version = "0.1.0"
authors = ["dev@nikaia.org"]

# Definiert das Standard-Verhalten bei `nikaia build`
# Optionen: "lite" (I/O optimized) oder "standard" (CPU optimized)
default-profile = "standard"

[dependencies]
# Standard Nikaia Libraries
http-server = "1.2"

# Import von nativen Rust Crates (via nikaia-bindgen)
[dependencies.regex]
type = "rust-crate"
version = "1.5"

[profiles.lite]
# Optimierungen für kleine Binary-Größe
opt-level = "z"
panic = "abort"

[profiles.standard]
# Optimierungen für Durchsatz
opt-level = 3
lto = true
```

#### 6.5.1. Reproduzierbarkeit und das Lockfile (nikaia.lock)
Das Lockfile speichert in Nikaia nicht nur die Versionen der Abhängigkeiten, sondern dient auch als Cache-Schlüssel für Compile-Time I/O (siehe Kapitel 4.9).
* **Asset Hashing:** Wenn ein Makro oder eine `grammar` eine externe Datei liest (z.B. `from "schema.sql"`), berechnet der Compiler den SHA256-Hash dieser Datei und schreibt ihn in `nikaia.lock`.
* **Cache-Invalidierung:** Bei nachfolgenden Builds prüft der Compiler, ob sich der Hash der Datei auf der Festplatte geändert hat.
    * Falls nein: Das Makro wird nicht neu ausgeführt (Instant Build).
    * Falls ja: Der Build wird invalidiert und neu getriggert.

### 6.6. Die WebAssembly (WASM) Synergie
Das "Lite Profil" von Nikaia besitzt eine natürliche Affinität zu WebAssembly. Da WASM (in seiner Basisform) ein lineares Speichermodell hat und oft in Single-Threaded-Host-Umgebungen (Browser Main-Thread, Edge Worker) läuft, ist das Lite-Profil das perfekte Match.

* **Kompilierung:** `nikaia build --profile=lite --target=wasm32-unknown`
* **Kein Overhead:** Da das Lite-Profil keine Atomics und keine OS-Mutexes generiert, ist der resultierende WASM-Code extrem kompakt und performant.
* **DOM-Interop:** Über die `dsl`-Schnittstelle (Kapitel 4.3) kann JavaScript-Code direkt inline eingebettet werden.

```nika
// main.nika (Lite Profil)
fn main() {
    let button = document.query_selector("#submit")
    
    // Die Closure wird in eine JS-Callback-Funktion umgewandelt
    button.add_event_listener("click", |_| {
        window.alert("Nikaia läuft im Browser!")
    })
}
```

### 6.7. Dokumentation (Doc Comments)
Nikaia unterstützt Markdown direkt in Kommentaren (`///`). Der Compiler generiert daraus statische HTML-Dokumentation. Code-Beispiele in der Dokumentation werden automatisch als Tests ausgeführt ("Doctests"), um sicherzustellen, dass die Doku nie veraltet.

---

## Kapitel 7: Typ-Erweiterungen und Kohärenz

Um die Lesbarkeit und den "Fluss" des Codes zu maximieren, erlaubt Nikaia die Erweiterung bestehender Typen um neue Methoden. Dies verhindert die Notwendigkeit von wortreichen Utility-Klassen und ermöglicht eine subjekt-orientierte Syntax (`data.process()` statt `process(data)`).

### 7.1. Extension Methods
Methoden können zu jedem beliebigen Typ hinzugefügt werden, einschließlich primitiver Typen (`i32`, `bool`) oder Typen aus externen Bibliotheken.

```nika
// Erweiterung des primitiven Typs `String`
impl String {
    // Fügt eine neue Methode hinzu, die prüft, ob der String wie eine E-Mail aussieht
    fn is_valid_email(&self) -> bool {
        return self.contains("@") && self.contains(".")
    }
    
    // Eine Methode, die den String transformiert (Ownership Transfer)
    fn normalize_email(self) -> String {
        return self.trim().to_lowercase()
    }
}

fn main() {
    let raw_input = "  User@Nikaia.DEV  "
    
    // Aufruf der Extension Methods
    if raw_input.is_valid_email() {
        let clean = raw_input.normalize_email()
        println("Bereinigte E-Mail: {clean}")
    }
}
```

### 7.2. Kohärenz und Sichtbarkeit
Um Namenskonflikte zu vermeiden (z.B. wenn zwei Bibliotheken eine `save()` Methode für `File` definieren), gelten strenge Sichtbarkeitsregeln:
* **Lokale Erweiterungen:** Ein `impl`-Block im selben Modul ist immer sichtbar.
* **Externe Erweiterungen:** Erweiterungen, die in anderen Modulen oder Bibliotheken definiert sind, sind nur sichtbar, wenn sie explizit importiert werden. Nikaia erlaubt benannte Extension-Blöcke für diesen Zweck.

```nika
// In Bibliothek A
pub extension JsonExtensions for String {
    fn to_json(&self) -> JsonValue { ... }
}

// In Bibliothek B (Nutzer)
use lib_a::JsonExtensions // Import macht `to_json` auf Strings verfügbar

fn process(s: String) {
    let json = s.to_json() 
}
```

---

## Kapitel 8: Die Standardbibliothek ("Batteries Included")

Im Gegensatz zu Sprachen, die eine minimale Standardbibliothek bevorzugen, verfolgt Nikaia den Ansatz der sofortigen Produktivität.

### 8.1. Universelle Module (Lite & Standard)
Diese Module basieren auf den Unified Types und funktionieren in beiden Compiler-Profilen identisch.

* `std::http`: Ein produktionsreifer HTTP/1.1 und HTTP/2 Client und Server.
    ```nika
    use std::http

    fn main() {
        // Startet einen Server auf Port 8080
        // In Lite: Single-Threaded Event Loop
        // In Standard: Multi-Threaded Work Stealing
        http::Server::new()
            .route("/", |_| "Hello World")
            .listen(":8080")
    }
    ```
* `std::json`: Performante Serialisierung und Deserialisierung. Nutzt Code-Generierung zur Kompilierzeit (ähnlich serde).
* `std::fs`: Dateisystemzugriff. Alle Operationen sind API-seitig blockierend gestaltet, werden aber vom Compiler in non-blocking I/O transformiert.
* `std::cli`: Parser für Kommandozeilenargumente, Environment-Variablen und Terminal-Farben (ANSI-Codes).
* `std::net`: Low-Level TCP/UDP Sockets.

### 8.2. Profil-Spezifische Module
Diese Module stehen zur Verfügung, warnen jedoch oder verhalten sich unterschiedlich, je nach Profil.

* `std::process`: Starten von Kindprozessen.
* `std::thread`: (Nur Standard-Profil) Erlaubt das explizite Starten von OS-Threads und thread-lokalem Speicher. Im Lite-Profil führt die Nutzung zu einem Kompilierfehler, wenn das Target keine Threads unterstützt (z.B. WASM).

---

## Anhang A: Fehler-Hierarchie

Nikaia unterscheidet strikt zwischen erwartbaren Fehlern (Laufzeitumgebung) und logischen Fehlern (Programmierfehler).

### A.1. Recoverable Errors (throws)
Fehler, die durch externe Umstände entstehen (Datei fehlt, Netzwerk down).
* **Mechanismus:** Müssen in der Funktionssignatur deklariert werden.
* **Behandlung:** Erzwungen durch den Compiler via `?{}` oder Weitergabe.

### A.2. Unrecoverable Errors (panic)
Fehler, die einen inkonsistenten Programmzustand anzeigen (Index Out of Bounds, Division durch Null, explizites `panic()`). Das Verhalten unterscheidet sich je nach Profil:

| Profil | Verhalten bei Panic | Konsequenz |
|---|---|---|
| **Lite** | Abort | Der gesamte Prozess wird sofort beendet. In WebAssembly führt dies zu einem Trap. Es gibt kein Unwinding. |
| **Standard** | Task Poisoning | Nur der betroffene Task (Green Thread) wird beendet. Der Worker-Thread fängt den Fehler ab (Fault Isolation). Ressourcen (`Locked<T>`), die der Task hielt, werden als "vergiftet" markiert, um den Zugriff auf korrupte Daten durch andere Threads zu verhindern. |

---

## Kapitel 9: Compiler-Architektur und Bootstrap (ADR-001)
**Version:** 0.0.3 (Draft)

Dieses Kapitel definiert die fundamentale technische Implementierung des Nikaia-Compilers (`nikaiac`). Es dokumentiert die bewusste Abkehr vom klassischen Transpiler-Ansatz zugunsten einer monolithischen Integration in das Rust-Compiler-Frontend ("Rustc Driver Pattern").

### 9.1. Architektur-Entscheidung: Der Driver-Ansatz
Nikaia wird nicht als externer Präprozessor implementiert, der `.rs` Dateien auf die Festplatte schreibt. Stattdessen fungiert `nikaiac` als ein Wrapper um `rustc`, der direkten Zugriff auf dessen interne APIs (`rustc_driver`, `rustc_interface`) nutzt.

**Der Kompilier-Fluss:**
1.  **Frontend (Nikaia):** Parsen der `.nika` Dateien in den Nikaia-AST.
2.  **Semantische Analyse:** Validierung von Profil-Constraints und Auflösung von Extensions.
3.  **Lowering (In-Memory):** Transformation des Nikaia-AST direkt in einen Rust `TokenStream`.
4.  **Injection:** Übergabe des TokenStreams an den Rust-Compiler-Prozess als virtuelle Eingabe.
5.  **Backend (Rustc):** Makro-Expansion, Type-Checking, Optimierung (LLVM) und Codegenerierung.

**Begründung:**
* **Makro-Support:** Der generierte Code muss vor der Makro-Expansion in den Compiler eintreten.
* **Configuration Injection:** Der Driver setzt basierend auf Anhang A dynamisch Flags (z.B. `-C panic=abort` für Lite).
* **Fehler-Mapping:** Nutzung interner Span-Strukturen, um Fehler direkt auf die `.nika`-Datei zu mappen.

### 9.2. Stabilität durch Version Pinning
Da die internen APIs von Rust instabil sind, erzwingt Nikaia eine strikte Kopplung an eine spezifische Compiler-Version. Jedes Nikaia-Release liefert eine `rust-toolchain.toml` Datei mit, die einen exakten Nightly-Build von Rust pinnt.

### 9.3. Das Lowering-Verfahren (Vibe to Tokens)
Der Kern des Compilers ist der `LoweringContext`, der Nikaia-Konstrukte in Rust-Tokens übersetzt.

**Beispiel: Transformation eines `spawn` Aufrufs**

*Input (Nikaia AST):*
`spawn || { do_work() }`

*Lowering Logik (Pseudocode im Compiler):*
```rust
fn lower_spawn(closure_body: NikaiaExpr) -> TokenStream {
    // 1. Profil-Check: Lite oder Standard?
    let spawner = if self.profile.is_lite() {
        quote! { tokio::task::spawn_local }
    } else {
        quote! { tokio::task::spawn }
    };

    // 2. Token-Generierung mit `quote!`
    // Wir injizieren `async move`, da Nikaia Tasks immer asynchrone State-Machines sind.
    quote! {
        #spawner(async move {
            #closure_body // Rekursives Lowering des Bodys
        })
    }
}
```

*Output (Rust TokenStream):*
`tokio::task::spawn(async move { do_work().await })`

#### 9.3.1. Lowering von Extension Methods (Auto-Traits)
Um die in Kapitel 7 definierten Ad-hoc Erweiterungen auf Rust abzubilden, wendet der Compiler die Strategie der "Synthetischen Traits" an.

```rust
// Output (Generierter Rust-TokenStream):
// Der Compiler generiert einen hygienischen Namen
trait __NikaiaExtension_String_GenId123 {
    fn is_mail(&self);
}
// Implementierung des Traits für den Zieltyp
impl __NikaiaExtension_String_GenId123 for String {
    fn is_mail(&self) { ... }
}
```

### 9.4. Error Mapping (Span Preservation)
Nikaia erzeugt Tokens nicht mit `Span::call_site()`, sondern konstruiert Spans, die auf die Positionen im `.nika` Quelltext zeigen. Dadurch unterstreicht die IDE bei Fehlern die korrekte Zeile im Nikaia-Code, nicht im generierten Rust-Code.

### 9.5. Bootstrapping Phasen
1.  **Stage 0:** Prototyp als regulärer Rust-Crate (syn/quote).
2.  **Stage 1:** Der Compiler wird in Nikaia neu geschrieben und von Stage 0 kompiliert (Self-Hosting).
3.  **Stage 2:** Der in Nikaia geschriebene Compiler kompiliert sich selbst.

---

## Kapitel 10: Die Implementierung des nikaiac Compilers
**Version:** 0.0.3 (Draft)

Der Compiler (`nikaiac`) ist selbst ein Nikaia-Programm ("Dogfooding").

### 10.1. Modul-Struktur
* `nikaiac::ast`: Datenmodell (AST).
* `nikaiac::parser`: Nikaia-Grammatik.
* `nikaiac::analysis`: Semantische Analyse und Method Resolution.
* `nikaiac::lowering`: Transformation AST -> TokenStream.
* `nikaiac::driver`: Interface zu `rustc_driver`.

### 10.2. Das Datenmodell (nikaiac::ast)
Nutzung von Enums und Unified Types (`Shared`).

```nika
// ast.nika
pub enum Expr {
    LiteralInt(i64),
    LiteralStr(String),
    Variable(String),
    
    // Binäre Operation: 1 + 2
    Binary { 
        op: BinOp, 
        left: Shared[Expr], 
        right: Shared[Expr] 
    },
    
    // Block: { ... }
    Block(Vec[Stmt]),
    
    // Async Control Flow
    Select(Vec[SelectBranch]),
    Spawn(Shared[Expr])
}
```

### 10.3. Der Parser (nikaiac::parser)
Nutzung der nativen `grammar` DSL.

```nika
// parser.nika
use crate::ast::{Expr, Stmt}

pub grammar CompilerGrammar {
    option recursion_limit = 256;

    // Startregel
    pub rule compilation_unit -> Vec[Stmt] = stmt()*

    rule stmt -> Stmt = 
        | import_stmt()
        | let_stmt()
        | expr_stmt()

    rule let_stmt -> Stmt = {
        "let" name:ident() "=" val:expr()
    } -> { Stmt::Let { name, type_hint: None, value: val } }
}
```

### 10.4. Semantische Analyse (nikaiac::analysis)
```nika
// analysis.nika
pub fn check_constraints(module: &Module, profile: Profile) throws CompileError {
    // Profil-Constraint Check
    if profile == Profile::Lite {
        for stmt in module.stmts {
            if let Stmt::Import { path } = stmt {
                if path.starts_with("std::thread") {
                    throw CompileError(
                        "Das Modul 'std::thread' ist im Lite-Profil verboten. " +
                        "Nutzen Sie 'spawn' für I/O-Concurrency."
                    )
                }
            }
        }
    }
    register_extensions(module)
}
```

### 10.5. Die Lowering-Engine (nikaiac::lowering)
Herzstück der Übersetzung. Nutzt ein internes `TokenBuilder` Interface.

```nika
// lowering.nika
use crate::ast
use crate::driver::Profile

pub struct LoweringContext {
    profile: Profile,
    tokens: TokenBuilder
}

impl LoweringContext {
    pub fn lower_spawn(self, task_body: ast::Expr) {
        let spawner_func = match self.profile {
            Profile::Lite => "tokio::task::spawn_local",
            Profile::Standard => "tokio::task::spawn"
        }

        self.tokens.push_path(spawner_func)
        self.tokens.push_group(Delimiter::Parenthesis, |inner| {
            inner.push_keyword("async")
            inner.push_keyword("move")
            inner.push_group(Delimiter::Brace, |block| {
                block.lower_expr_with_await(task_body)
            })
        })
    }
}
```

### 10.6. Testing und Verifikation
Snapshot Tests für den generierten TokenStream.

```nika
// tests/codegen_test.nika
test "Spawn Lite Profile Generierung" {
    // Input
    let snippet = "spawn { 1 + 1 }"
    
    // Action
    let output = nikaiac::compile_snippet(snippet, Profile::Lite)
    
    // Assertion (Prüft, ob der korrekte Tokio-Scheduler gewählt wurde)
    assert output.contains("tokio::task::spawn_local")
    assert !output.contains("tokio::task::spawn(") 
}
```

### 10.7. Praktische Umsetzung des Bootstraps
Wie in Kapitel 9.5 definiert, erfolgt der Bau in Stufen (Stage 0 -> Stage 1 -> Stage 2). Das Resultat von Stage 2 muss in der Lage sein, Nikaia-Standardbibliotheken und Nutzer-Code fehlerfrei zu verarbeiten.

---

### Schlusswort
Systemprogrammierung war lange Zeit eine Disziplin des "Leidens für die Performance". Wir akzeptierten Segfaults in C oder Borrow-Checker-Kämpfe in Rust als den Preis für Geschwindigkeit. Nikaia ist der Beweis – zumindest auf dem Papier –, dass wir leiden, weil unsere Abstraktionen veraltet sind, nicht weil das Problem es erfordert.

Wir laden dazu ein, diesen Entwurf zu forkern, zu kritisieren und zu bauen. Happy Vibe Coding.
