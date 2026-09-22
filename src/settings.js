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
  const lidModes = [...document.querySelectorAll('input[name="lid-mode"]')];
  const backpack = $("backpack");
  const deskMode = $("desk-mode");
  const lockOnOpen = $("lock-on-open");
  const batteryThreshold = $("battery-threshold");
  const warnBeforeEnd = $("warn-before-end");
  const shortcutToggle = $("shortcut-toggle");
  const shortcutScreenOff = $("shortcut-screen-off");
  const updateStatus = $("update-status");
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

    // Coperchio: solo sui portatili. Con un criterio aziendale si vede, ma è
    // disattivato e lo dice.
    $("lid-card").hidden = !s.laptop;
    $("lid-policy").hidden = !s.policyManaged;
    for (const el of $("lid-controls").querySelectorAll("input, select")) {
      el.disabled = s.policyManaged;
    }
    for (const radio of lidModes) radio.checked = radio.value === s.lidMode;
    $("backpack-field").hidden = s.lidMode !== "always";
    fillSelect(backpack, s.backpackChoices, s.backpackMinutes);
    deskMode.checked = s.deskMode;
    lockOnOpen.checked = s.lockOnLidOpen;
    $("windows-lid").textContent = s.windowsLid;
    $("lid-held").hidden = !s.lidHeld;
    $("restore-lid").hidden = !s.lidHeld;
    $("lid-error").hidden = !s.lidError;
    $("lid-error").textContent = s.lidError || "";

    $("battery-card").hidden = !s.hasBattery;
    fillSelect(batteryThreshold, s.batteryChoices, s.batteryThreshold);

    warnBeforeEnd.checked = s.warnBeforeEnd;
    fillSelect(shortcutToggle, s.shortcutChoices, s.shortcutToggle);
    fillSelect(shortcutScreenOff, s.shortcutChoices, s.shortcutScreenOff);
    $("shortcut-error").hidden = !s.shortcutError;
    $("shortcut-error").textContent = s.shortcutError || "";
    if (s.updateVersion) {
      showUpdate(I18n.t("settings.update_available", { version: s.updateVersion }), true);
    }
  }

  function showUpdate(text, installable) {
    updateStatus.hidden = false;
    updateStatus.textContent = text;
    $("update-install").hidden = !installable;
  }

  // Le opzioni (e le loro etichette tradotte) arrivano da Rust: la pagina
  // non sa quali valori sono ammessi.
  function fillSelect(select, choices, value) {
    const key = choices.map((c) => c.value + ":" + c.label).join("|");
    if (select.dataset.key !== key) {
      select.dataset.key = key;
      select.replaceChildren(
        ...choices.map((c) => {
          const option = document.createElement("option");
          option.value = String(c.value);
          option.textContent = c.label;
          return option;
        }),
      );
    }
    select.value = String(value);
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
  for (const radio of lidModes) {
    radio.addEventListener("change", () => save({ lidMode: radio.value }));
  }
  backpack.addEventListener("change", () => save({ backpackMinutes: Number(backpack.value) }));
  deskMode.addEventListener("change", () => save({ deskMode: deskMode.checked }));
  lockOnOpen.addEventListener("change", () => save({ lockOnLidOpen: lockOnOpen.checked }));
  batteryThreshold.addEventListener("change", () =>
    save({ batteryThreshold: Number(batteryThreshold.value) }),
  );
  warnBeforeEnd.addEventListener("change", () => save({ warnBeforeEnd: warnBeforeEnd.checked }));
  shortcutToggle.addEventListener("change", () => save({ shortcutToggle: shortcutToggle.value }));
  shortcutScreenOff.addEventListener("change", () =>
    save({ shortcutScreenOff: shortcutScreenOff.value }),
  );
  $("update-check").addEventListener("click", async () => {
    showUpdate(I18n.t("settings.update_checking"), false);
    try {
      const version = await invoke("check_updates");
      if (version) {
        showUpdate(I18n.t("settings.update_available", { version }), true);
      } else {
        showUpdate(I18n.t("settings.update_none"), false);
      }
    } catch (err) {
      showUpdate(I18n.t("settings.update_error", { error: String(err) }), false);
    }
  });
  $("update-install").addEventListener("click", async () => {
    try {
      await invoke("install_update");
    } catch (err) {
      showUpdate(String(err), false);
    }
  });
  $("restore-lid").addEventListener("click", async () => {
    await fill(await invoke("restore_lid_now"));
  });

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

  // Lo stato è cambiato altrove (lingua, sessione, coperchio): si ridisegna.
  listen("moka://state", async () => {
    await fill(await invoke("get_settings"));
  });

  (async () => {
    try {
      await fill(await invoke("get_settings"));
    } finally {
      invoke("settings_ready");
    }
  })();
})();
