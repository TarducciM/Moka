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
  const presence = $("presence");
  const rulesList = $("rules-list");
  const ruleForm = $("rule-form");
  const ruleKind = $("rule-kind");
  const ruleExe = $("rule-exe");
  const ruleKbps = $("rule-kbps");
  const ruleCpu = $("rule-cpu");
  const ruleDays = $("rule-days");
  const ruleFrom = $("rule-from");
  const ruleTo = $("rule-to");
  const ruleMode = $("rule-mode");
  const ruleThen = $("rule-then");
  const ruleError = $("rule-error");
  let toastTimer = null;
  let rulesKey = "";
  let daysKey = "";

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

    presence.checked = s.presence;
    renderRules(s);
  }

  // ------------------------------------------------------------- regole

  const SVG = "http://www.w3.org/2000/svg";
  function icon(paths) {
    const svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    for (const d of paths) {
      const path = document.createElementNS(SVG, "path");
      path.setAttribute("d", d);
      svg.append(path);
    }
    return svg;
  }

  function renderRules(s) {
    $("rules-empty").hidden = s.rules.length > 0;
    $("rules-paused").hidden = !s.rulesPaused;
    $("rules-paused-text").textContent = s.rulesPaused || "";
    $("rules-pause").hidden = Boolean(s.rulesPaused) || s.rules.length === 0;

    // Le scelte del modulo: tipi, soglie e giorni arrivano da Rust.
    fillSelect(ruleKind, s.ruleKinds, ruleKind.value || s.ruleKinds[0].value);
    fillSelect(ruleKbps, s.downloadChoices, ruleKbps.value || s.downloadChoices[1].value);
    fillSelect(ruleCpu, s.cpuChoices, ruleCpu.value || s.cpuChoices[1].value);
    fillSelect(ruleThen, s.thenChoices, ruleThen.value || "none");
    const dk = s.dayLabels.join("|");
    if (dk !== daysKey) {
      const checked = [...ruleDays.querySelectorAll("input")].map((i) => i.checked);
      daysKey = dk;
      ruleDays.replaceChildren(
        ...s.dayLabels.map((label, i) => {
          const wrap = document.createElement("label");
          wrap.className = "day";
          const input = document.createElement("input");
          input.type = "checkbox";
          input.value = String(i);
          // Di default i giorni feriali.
          input.checked = checked.length ? checked[i] : i < 5;
          const span = document.createElement("span");
          span.textContent = label;
          wrap.append(input, span);
          return wrap;
        }),
      );
    }
    showKindFields();

    // L'elenco si ricostruisce solo se cambiano le regole (o la lingua), non
    // quando una regola comincia o smette di valere: quello è solo il badge.
    const key = JSON.stringify([
      s.lang,
      s.thenChoices,
      s.rules.map((r) => [r.id, r.enabled, r.mode, r.then, r.title, r.detail]),
    ]);
    if (key !== rulesKey) {
      rulesKey = key;
      rulesList.replaceChildren(...s.rules.map((r) => ruleItem(r, s)));
    }
    for (const r of s.rules) {
      const live = rulesList.querySelector(`[data-live="${r.id}"]`);
      if (live) live.hidden = !(r.active && r.enabled);
    }
  }

  function ruleItem(r, s) {
    const name = r.detail ? `${r.title} (${r.detail})` : r.title;
    const li = document.createElement("li");
    li.className = "rule";
    li.classList.toggle("is-off", !r.enabled);

    const head = document.createElement("div");
    head.className = "rule-head";
    const text = document.createElement("span");
    text.className = "rule-text";
    const title = document.createElement("span");
    title.className = "rule-title";
    title.textContent = r.title;
    text.append(title);
    if (r.detail) {
      const detail = document.createElement("small");
      detail.textContent = r.detail;
      text.append(detail);
    }
    const live = document.createElement("span");
    live.className = "rule-live";
    live.dataset.live = String(r.id);
    live.textContent = I18n.t("settings.rule_live");
    live.hidden = !(r.active && r.enabled);
    const enabled = document.createElement("input");
    enabled.type = "checkbox";
    enabled.checked = r.enabled;
    enabled.setAttribute("aria-label", I18n.t("settings.rule_enabled", { rule: name }));
    enabled.addEventListener("change", () => ruleCall("update_rule", { id: r.id, enabled: enabled.checked }));
    head.append(text, live, enabled);

    const opts = document.createElement("div");
    opts.className = "rule-opts";
    const mode = document.createElement("select");
    mode.setAttribute("aria-label", `${I18n.t("settings.rule_keep")}: ${name}`);
    for (const value of ["system", "display"]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = I18n.t(`mode.${value}`);
      mode.append(option);
    }
    mode.value = r.mode;
    mode.addEventListener("change", () => ruleCall("update_rule", { id: r.id, mode: mode.value }));
    const then = document.createElement("select");
    then.setAttribute("aria-label", `${I18n.t("settings.rule_then")}: ${name}`);
    fillSelect(then, s.thenChoices, r.then);
    then.addEventListener("change", () => ruleCall("update_rule", { id: r.id, then: then.value }));
    const del = document.createElement("button");
    del.type = "button";
    del.className = "icon-btn rule-delete";
    const label = I18n.t("settings.rule_delete", { rule: name });
    del.setAttribute("aria-label", label);
    del.title = label;
    del.append(icon(["M4.5 7h15", "M9.5 7V4.5h5V7", "M6.5 7l1 12.5h9l1-12.5", "M10.5 11v5M13.5 11v5"]));
    del.addEventListener("click", () => ruleCall("delete_rule", { id: r.id }, "settings.rule_deleted"));
    opts.append(labeled("settings.rule_keep", mode), labeled("settings.rule_then", then), del);

    li.append(head, opts);
    return li;
  }

  // "Niente" da solo non dice niente: ogni tendina ha la sua etichetta visibile.
  function labeled(key, select) {
    const wrap = document.createElement("label");
    wrap.className = "rule-opt";
    const text = document.createElement("small");
    text.textContent = I18n.t(key);
    wrap.append(text, select);
    return wrap;
  }

  async function ruleCall(command, args, okKey = "settings.saved") {
    try {
      await fill(await invoke(command, args));
      showToast(I18n.t(okKey));
      return true;
    } catch (message) {
      showToast(String(message));
      return false;
    }
  }

  function showKindFields() {
    const kind = ruleKind.value;
    $("rule-exe-field").hidden = kind !== "process";
    $("rule-kbps-field").hidden = kind !== "download";
    $("rule-cpu-field").hidden = kind !== "cpu";
    $("rule-schedule-field").hidden = kind !== "schedule";
  }

  const minutes = (value) => {
    const [h, m] = value.split(":").map(Number);
    return Number.isInteger(h) && Number.isInteger(m) ? h * 60 + m : null;
  };

  // La regola come la vuole Rust: { kind, ...campi, mode, then }. Un campo
  // sbagliato torna come messaggio già tradotto.
  function readRule() {
    const rule = { kind: ruleKind.value, mode: ruleMode.value, then: ruleThen.value };
    switch (rule.kind) {
      case "process":
        rule.exe = ruleExe.value.trim();
        if (!rule.exe) return { error: I18n.t("settings.rule_invalid"), focus: ruleExe };
        break;
      case "download":
        rule.kbps = Number(ruleKbps.value);
        break;
      case "cpu":
        rule.percent = Number(ruleCpu.value);
        break;
      case "schedule": {
        rule.days = [...ruleDays.querySelectorAll("input:checked")].reduce(
          (bits, input) => bits | (1 << Number(input.value)),
          0,
        );
        rule.from = minutes(ruleFrom.value);
        rule.to = minutes(ruleTo.value);
        if (!rule.days) {
          return { error: I18n.t("settings.rule_days_empty"), focus: ruleDays.querySelector("input") };
        }
        if (rule.from === null || rule.to === null || rule.from === rule.to) {
          return { error: I18n.t("settings.rule_same_time"), focus: ruleTo };
        }
        break;
      }
      default:
        break;
    }
    return { rule };
  }

  function showRuleError(text, focus) {
    ruleError.hidden = !text;
    ruleError.textContent = text || "";
    ruleExe.toggleAttribute("aria-invalid", Boolean(text) && focus === ruleExe);
    if (focus) focus.focus();
  }

  function openRuleForm(open) {
    ruleForm.hidden = !open;
    $("rule-open").hidden = open;
    showRuleError("");
    if (!open) {
      $("rule-open").focus();
      return;
    }
    ruleKind.focus();
    // I programmi aperti adesso, come suggerimenti: il campo resta libero.
    invoke("list_processes")
      .then((names) => {
        $("rule-exe-list").replaceChildren(
          ...names.map((name) => {
            const option = document.createElement("option");
            option.value = name;
            return option;
          }),
        );
      })
      .catch(() => {});
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
  presence.addEventListener("change", () => save({ presence: presence.checked }));
  $("rule-open").addEventListener("click", () => openRuleForm(true));
  $("rule-cancel").addEventListener("click", () => openRuleForm(false));
  ruleKind.addEventListener("change", showKindFields);
  // Un errore vale per ciò che c'era scritto: appena si corregge, sparisce.
  ruleForm.addEventListener("input", () => showRuleError(""));
  ruleForm.addEventListener("change", () => showRuleError(""));
  ruleForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const { rule, error, focus } = readRule();
    if (error) {
      showRuleError(error, focus);
      return;
    }
    try {
      await fill(await invoke("add_rule", { rule }));
      ruleExe.value = "";
      openRuleForm(false);
      showToast(I18n.t("settings.rule_added"));
    } catch (message) {
      showRuleError(String(message), ruleKind);
    }
  });
  $("rules-pause").addEventListener("click", async () => {
    await fill(await invoke("pause_rules", { minutes: 60 }));
  });
  $("rules-resume").addEventListener("click", async () => {
    await fill(await invoke("resume_rules"));
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
    if (event.key !== "Escape") return;
    // Esc chiude prima il modulo della regola, poi la finestra.
    if (!ruleForm.hidden) {
      openRuleForm(false);
      return;
    }
    invoke("close_settings");
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
