// Lingua del sito: italiano o inglese. Ogni pagina contiene entrambe le
// versioni (data-lang="it" / "en"); qui se ne mostra una. La scelta si
// ricorda nel localStorage del browser (niente cookie, niente server), e
// senza JavaScript restano visibili tutte e due.

(() => {
  const KEY = "moka-site-lang";
  const root = document.documentElement;
  root.classList.add("js");

  function saved() {
    try {
      return localStorage.getItem(KEY);
    } catch {
      return null;
    }
  }

  function pick() {
    const s = saved();
    if (s === "it" || s === "en") return s;
    return (navigator.language || "").toLowerCase().startsWith("it") ? "it" : "en";
  }

  function apply(lang) {
    root.lang = lang;
    for (const el of document.querySelectorAll("[data-lang]")) {
      el.classList.toggle("is-current", el.dataset.lang === lang);
    }
    for (const btn of document.querySelectorAll("[data-lang-switch]")) {
      btn.setAttribute("aria-pressed", String(btn.dataset.langSwitch === lang));
    }
    const title = document.querySelector(`meta[name="title-${lang}"]`);
    if (title) document.title = title.content;
  }

  document.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-lang-switch]");
    if (!btn) return;
    const lang = btn.dataset.langSwitch;
    try {
      localStorage.setItem(KEY, lang);
    } catch {
      // senza storage la scelta vale solo per questa pagina
    }
    apply(lang);
  });

  for (const el of document.querySelectorAll(".year")) {
    el.textContent = String(new Date().getFullYear());
  }
  apply(pick());
})();
