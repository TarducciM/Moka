; Moka: agganci dell'installer NSIS (bundle.windows.nsis.installerHooks).
;
; La regola: quello che Moka cambia in Windows non resta MAI cambiato. Sono
; due cose: l'impostazione "Quando chiudo il coperchio" e la pausa degli
; aggiornamenti di Windows (0.7.0). Chiudere Moka a forza con una delle due
; attive (è ciò che fa CheckIfAppIsRunning, subito dopo questi agganci) le
; lascerebbe lì, e alla disinstallazione anche RunOnce punterebbe a un
; eseguibile appena cancellato. Quindi prima si chiude Moka in modo pulito
; (--quit rimette tutto), poi i due ripristini riparano un eventuale registro
; lasciato da un crash.

!macro MOKA_CLOSE_CLEANLY
  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --quit'
    Pop $0
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-lid'
    Pop $0
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-updates'
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Aggiornamento o reinstallazione sopra una Moka già installata.
  !insertmacro MOKA_CLOSE_CLEANLY
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; La scelta della pagina "Attività aggiuntive" passa dall'app stessa
  ; (--enable-autostart), lo stesso codice dell'interruttore nelle
  ; Impostazioni: non può divergere. Eseguita come l'utente vero, non come
  ; l'installer, perché la chiave di avvio sta nel suo HKCU. Vuota nelle
  ; installazioni silenziose: un aggiornamento non la tocca mai.
  ${If} $EnableAutostart == 1
    nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "--enable-autostart"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro MOKA_CLOSE_CLEANLY
!macroend
