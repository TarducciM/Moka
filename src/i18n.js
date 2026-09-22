// Traduzioni delle pagine. I testi stanno in locales/<lingua>.json, gli stessi
// file che Rust include per menu e tooltip: una sola fonte.
//
// Nell'HTML: data-i18n (testo), data-i18n-title, data-i18n-aria (aria-label).
// Nel JS: I18n.t("chiave", { nome: valore }) per i segnaposto {nome}.
//
// Una chiave mancante non dà errori: mostra la chiave stessa. Per questo
// tests/i18n.test.mjs controlla che ogni chiave usata esista in tutte le lingue.

window.I18n = (() => {
  let dict = {};
  let lang = null;

  async function load(next) {
    if (next === lang) return;
    const response = await fetch(`locales/${next}.json`);
    dict = await response.json();
    lang = next;
    document.documentElement.lang = next;
  }

  function t(key, vars) {
    let text = Object.prototype.hasOwnProperty.call(dict, key) ? dict[key] : key;
    if (vars) {
      for (const [name, value] of Object.entries(vars)) {
        text = text.split(`{${name}}`).join(String(value));
      }
    }
    return text;
  }

  function apply(root = document) {
    for (const el of root.querySelectorAll("[data-i18n]")) {
      el.textContent = t(el.dataset.i18n);
    }
    for (const el of root.querySelectorAll("[data-i18n-title]")) {
      el.title = t(el.dataset.i18nTitle);
    }
    for (const el of root.querySelectorAll("[data-i18n-aria]")) {
      el.setAttribute("aria-label", t(el.dataset.i18nAria));
    }
  }

  return {
    load,
    t,
    apply,
    get lang() {
      return lang;
    },
  };
})();
