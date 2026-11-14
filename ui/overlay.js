const pill = document.getElementById("pill");

function show(text) {
  pill.textContent = text || "";
}

const api = window.__TAURI__;
if (api) {
  api.event.listen("overlay-text", (event) => show(event.payload));
}
