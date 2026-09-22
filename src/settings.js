// Impostazioni. Ogni modifica si salva subito, campo per campo: niente
// pulsante "Salva" da dimenticare. Un errore di Rust arriva già tradotto e si
// mostra accanto al campo, senza toccare il resto del modulo (trappola 8: un
// errore non deve mai sostituire il form).

(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const { openUrl } = window.__TAURI__.opener;

  const $ = (id) => document.getElementById(id);
  const language = $("language");
  const autostart = $("autostart");
  const durations = $("durations");
  const durationsError = $("durations-error");
  const toast = $("toast");
  const leftClick = [...document.querySelectorAll('input[name="left-click"]')];
  let toastTimer = null;

  async function fill(s) {
    if (I18n.lang !== s.lang) {
      await I18n.load(s.lang);
      I18n.apply();
    }
    language.value = s.language;
    autostart.checked = s.autostart;
    for (const radio of leftClick) radio.checked = radio.value === s.leftClick;
    if (document.activeElement !== durations) durations.value = s.durationsText;
    $("version").textContent = I18n.t("settings.version", { version: s.version });
  }

  async function save(patch) {
    try {
      const s = await invoke("update_settings", { patch });
      await fill(s);
      showToast(I18n.t("settings.saved"));
      return true;
    } catch (message) {
      showToast(String(message));
      return false;
    }
  }

  function showToast(text) {
    toast.textContent = text;
    toast.classList.add("is-visible");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove("is-visible"), 1600);
  }

  language.addEventListener("change", () => save({ language: language.value }));
  autostart.addEventListener("change", () => save({ autostart: autostart.checked }));
  for (const radio of leftClick) {
    radio.addEventListener("change", () => save({ leftClick: radio.value }));
  }

  async function saveDurations() {
    try {
      const s = await invoke("update_settings", { patch: { durationsText: durations.value } });
      durationsError.hidden = true;
      durations.removeAttribute("aria-invalid");
      durations.value = s.durationsText;
      await fill(s);
      showToast(I18n.t("settings.saved"));
    } catch (message) {
      durationsError.textContent = String(message);
      durationsError.hidden = false;
      durations.setAttribute("aria-invalid", "true");
    }
  }
  durations.addEventListener("change", saveDurations);
  durations.addEventListener("keydown", (event) => {
    if (event.key === "Enter") durations.blur();
  });

  // I link si aprono nel browser, non dentro la finestra.
  document.addEventListener("click", (event) => {
    const link = event.target.closest("a[href^='http']");
    if (!link) return;
    event.preventDefault();
    openUrl(link.href).catch(() => {});
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") invoke("close_settings");
  });

  // Lingua cambiata altrove: si ridisegna.
  listen("moka://state", async (event) => {
    if (event.payload.lang !== I18n.lang) {
      await fill(await invoke("get_settings"));
    }
  });

  (async () => {
    try {
      await fill(await invoke("get_settings"));
    } finally {
      invoke("settings_ready");
    }
  })();
})();
