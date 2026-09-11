# Nikaia: Philosophy & Origin

**For Nika.**
*Because the future belongs to those who build it.*

---

## 1. The Name and the Heart
At the centre is **Nika** — my daughter's name. Everything else comes after that.

The name carries more than one lineage, and the deeper one is not the Greek: in Persian, *nik*
means **good** — virtuous, good the way a person is good and not the way a product is. The
motto is that name with a verb added: **Good wins.**

Not over anyone. What the sentence denies is something else: that good and fast are opposites,
and that being decent to the person writing the code has to be paid for at runtime. Here it is
the other way round. Because the language never makes you write `Arc`, lifetimes or lock orders,
those decisions belong to the compiler — and only because they belong to it can it choose `Rc`
where nothing of yours runs at once and `Arc` where it does, order the locks, and infer borrow contracts across a whole
program. A stricter language would have to take your word for it. Good is not the price of fast
here; it is the reason for it.

The ancient city of **Nikaia** (Νίκαια), which the name also points to, supplies the second
image — more on that in the next section.

This language is dedicated to her. It is an attempt to leave behind a technological world shaped
less by unnecessary hurdles and more by the freedom to create.

Because so much of software development costs effort that has nothing to do with the actual
problem: the compiler that holds you up, race conditions that only surface under load, the limit
of what one person can hold in their head. This is not a war, and nobody has to be defeated for
it. **Nikaia stands for not conquering that complexity, but moving it where it belongs: into the
compiler.**

## 2. The History: The End of the Schism
The ancient city of Nikaia is known for its **council** — a place of consensus. We are living
through a schism in programming today:

* The **"scripting faction"** (Python, JS): fast, flexible, but often fragile.
* The **"systems faction"** (Rust, C++): powerful, safe, but often cognitively heavy.

Nikaia is the technical council. It ends the split with a **Unified Core Architecture**.

## 3. The Agora and the Swarm
Nikaia needs two images, and only one of them is a place.

1.  **The Marketplace (Agora) — nothing of yours running at once:**
    One square, in constant movement. Trade, exchange, flow. Nobody stands still waiting for
    anybody else: everything is "non-blocking".
    *Optimised for:* I/O density, web services, rapid prototyping.
2.  **The Swarm — every core busy:**
    Not a place, and not a fortress. Ask what concurrency actually looks like and the answer is
    a swarm: no centre, no commander, no walls to defend. Every worker takes what is in front of
    it, and when it runs out it takes work from a neighbour. The order comes from the rules
    everyone follows, not from anyone giving orders — which is precisely what a work-stealing
    runtime is, and why safety here is a property of the rules rather than of everybody's
    discipline.
    *Optimised for:* compute power, thread safety, every core busy.

## 4. The Manifesto

For a long time we believed we had to choose.

We built our skyscrapers on sand because the concrete was too hard to mix. We wrote software
that felt good and collapsed in the night. Or we forged systems out of pure steel that lasted
forever, but whose construction cost us our joy.

We accepted the dogma: *"Simple is slow. Fast is hard."*

And then we asked the question: what if the weight is not in the tool, but in the way we hold
it? What if the compiler is not our overseer, but our architect?

We called the project **Nikaia**.

Because intent should count again, and not implementation. Because it tears down the walls
between the code we dream and the code the machine understands.

**Nikaia: Good wins.**
