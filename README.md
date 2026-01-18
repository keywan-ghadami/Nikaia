<div align="center">
  <img src="1768075880760.jpg" alt="Nikaia Logo" width="300" />
  <h1>N I K A I A</h1>
  <p><strong>Build the Victory.</strong></p>
  
  <p>
    <a href="#-philosophy">Philosophy</a> •
    <a href="#-the-promise">The Promise</a> •
    <a href="#-profiles">Profiles</a> •
    <a href="#-example">Example</a> •
    <a href="SPECIFICATION.md">Specification</a>
    <a href="https://gemini.google.com/gem/1T8viw7ZHA0TwDZDhr6h1mgRBVnw3aTNP?usp=sharing">Gemini explains Nikaia</a>
  </p>

  ![Version](https://img.shields.io/badge/version-0.0.3-blue.svg)
  ![Status](https://img.shields.io/badge/status-experimental-orange.svg)
  ![License](https://img.shields.io/badge/license-Apache_2.0-blue.svg)
</div>

---

## ⚡ What is Nikaia?

**Nikaia** is a *Vibe Coding Experiment*. It attempts to bridge the gap between the developer joy of scripting languages (Python, TS) and the raw power and safety of systems languages (Rust, C++).

We don't believe you should have to choose.
Nikaia features a **Unified Core Architecture**: You write simple, linear code ("Direct Style"), and the compiler decides based on your chosen profile whether to generate a lightweight event loop or a massively parallel thread pool.

> **"We no longer write code to satisfy the computer. We write Nikaia to win."**
>
> 👉 [Read the Manifesto and the origin story](MANIFESTO.md)

---

## 💎 The Promise

1.  **Async by Default:** No `async`, no `await`, no callback hell. The compiler generates state machines invisibly in the background.
2.  **Unified Types:** Write `Shared[T]`. In the **Lite** profile, it compiles to `Rc` (fast); in the **Standard** profile, it becomes `Arc` (thread-safe).
3.  **Zero Color:** A function is just a function. No fragmentation of the ecosystem into synchronous and asynchronous worlds.
4.  **No Garbage Collection:** Deterministic resource management via RAII and ownership, but without the pain.

---

## 🎛 Two Profiles, One Language

Nikaia adapts to your problem, not the other way around. Define the profile in your `nikaia.toml`:

### 🟢 Nikaia Lite (The I/O Engine)
* **Target:** Node.js alternative, Go, WASM.
* **Architecture:** Single-Threaded Event Loop.
* **Benefit:** No race conditions by design, maximum I/O density, minimal memory footprint.
* **Use Case:** Microservices, Web Servers, CLI Tools, Edge Workers.

### 🔵 Nikaia Standard (The Compute Engine)
* **Target:** Rust, C++ alternative.
* **Architecture:** Multi-Threaded Work-Stealing Runtime.
* **Benefit:** Utilizes all CPU cores, mathematically proven thread safety via borrow checking.
* **Use Case:** High-Performance Computing, Game Engines, Complex Backend Systems.

---

## 💻 Code Example

A simple HTTP server demonstrating how Nikaia handles concurrency without syntax noise:

```nika
use std::http
use std::fs

// `throws` replaces Result-Unwrapping. Errors bubble up automatically.
fn handle_request(req: http::Request) throws IoError {
    // Looks synchronous, but is non-blocking I/O (Suspension Point)
    let data = fs::read_string("index.html")
    return req.respond(200, data)
}

fn main() {
    println("Starting Nikaia Server on :8080")
    
    // `spawn` behaves polymorphically:
    // - Lite: Green Thread on Main Loop
    // - Standard: Task on Thread Pool
    spawn || {
        http::Server::new()
            .route("/", handle_request)
            .listen(":8080")
    }

    // No `await` needed. The process stays alive.
}
```

---

## 🛠 Roadmap to 0.1.0

This is currently a **Specification (Version 0.0.4)**. We are in the bootstrap phase.

- [x] **Spec 0.0.3:** Definition of Syntax, Profiles, and Unified Types.
- [x] **Manifesto:** Defining the soul and philosophy of the project.
- [ ] **Bootstrap Compiler:** A transpiler written in Rust (Stage 0).
- [ ] **Runtime Integration:** Binding `tokio` (Current-Thread & Thread-Pool).
- [ ] **Self-Hosting:** The compiler compiles itself.

---

## ❤️ Dedication

This project is dedicated to my daughter **Nika**.
Because the future belongs to those who build it—without unnecessary hurdles.

---

## ⚖️ Intellectual Property & Governance

Nikaia introduces the **Unified Core Architecture**, a novel approach to compile-time orchestration, deterministic concurrency (`access_all`), and profile-based runtime transformation.

**For Corporate Entities & Implementers:**
This repository establishes public **Prior Art** for these architectural concepts, ensuring they remain unencumbered for the open-source ecosystem. 

While Nikaia is released under the **Apache 2.0 License**, the project is designed with long-term governance in mind to prevent proprietary fragmentation. We actively invite organizations interested in adopting, extending, or standardizing these concepts to join us as **Founding Partners** rather than attempting parallel implementations.

The architecture is complex; let's build the standard together.

---

## 📄 License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for details.

<div align="center">
  <sub>Designed with Vibe. Built for Victory.</sub>
</div>

