// Curva website: the exploded stack and the download panel.
// Everything here is an extra: without JavaScript the plates still link, the panel shows the
// first plate, and every download link goes to the Releases page.
(function () {
  var root = document.documentElement;
  var isSpec = document.body.classList.contains("spec");
  var plates = Array.prototype.slice.call(document.querySelectorAll("[data-plate]"));
  var items = Array.prototype.slice.call(document.querySelectorAll("[data-info]"));
  var scene = document.querySelector(".scene");

  // Light and dark: the button in the top bar (theme.js does the work).
  var tbtn = document.querySelector("[data-theme-toggle]");
  if (tbtn && window.curvaTheme) tbtn.addEventListener("click", window.curvaTheme.toggle);

  // The stack is drawn 560 x 700 and scaled to the room it has: the width of its column, and
  // never taller than the window.
  function fit() {
    if (!scene) return;
    var w = scene.getBoundingClientRect().width;
    var top = document.querySelector(".top");
    var room = window.innerHeight - (top ? top.offsetHeight : 0) - 40;
    var k = Math.min(w / 560, room / 700, 1.3);
    root.style.setProperty("--k", Math.max(0.45, k).toFixed(3));
  }
  fit();
  window.addEventListener("resize", fit);
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(fit);

  // Sections fade up as they scroll into view.
  var bands = Array.prototype.slice.call(document.querySelectorAll(".band"));
  if ("IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } });
    }, { rootMargin: "0px 0px -10% 0px" });
    bands.forEach(function (b) { io.observe(b); });
  } else {
    bands.forEach(function (b) { b.classList.add("in"); });
  }

  // Moving the mouse up pulls the plates apart; down packs them together.
  var max = Math.round((isSpec ? 430 : 450) / Math.max(1, plates.length - 1));
  var min = isSpec ? 18 : 20;
  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!reduced) {
    // Only near the stack (within 80 px of it); farther away it settles back to rest.
    var scene = document.querySelector(".scene"), near = 80;
    window.addEventListener("mousemove", function (e) {
      if (!scene) return;
      var r = scene.getBoundingClientRect();
      var inside = e.clientX > r.left - near && e.clientX < r.right + near && e.clientY > r.top - near && e.clientY < r.bottom + near;
      if (!inside) { root.style.setProperty("--gap", rest + "px"); return; }
      var t = Math.max(0, Math.min(1, (e.clientY - r.top) / r.height));
      root.style.setProperty("--gap", (max - t * (max - min)).toFixed(0) + "px");
    }, { passive: true });
  }
  // Without a mouse (phones, tablets) the plates start spread out so every name can be read.
  var touch = window.matchMedia("(hover: none)").matches;
  var rest = Math.round(touch ? max * (isSpec ? 0.75 : 0.6) : isSpec ? max * 0.45 : 80);
  root.style.setProperty("--gap", rest + "px");

  // The plate under the mouse (or keyboard focus) shows in the panel on the right.
  function show(i) {
    items.forEach(function (el) { el.classList.toggle("on", el.getAttribute("data-info") === String(i)); });
  }
  plates.forEach(function (p) {
    var i = p.getAttribute("data-plate");
    p.addEventListener("mouseenter", function () { show(i); });
    p.addEventListener("focus", function () { show(i); });
    p.addEventListener("touchstart", function () { show(i); }, { passive: true });
  });

  // Download: the newest release from GitHub, with the visitor's system chosen first.
  // Take the newest release that isn't a draft, as the app's updater does.
  var API = "https://api.github.com/repos/curva3d/curva/releases?per_page=10";
  var match = {
    mac: function (n) { return /\.dmg$/i.test(n); },
    win: function (n) { return /\.exe$/i.test(n); },
    linux: function (n) { return /\.appimage$/i.test(n); }
  };
  var names = { mac: "macOS", win: "Windows", linux: "Linux" };
  var files = {};
  var btn = document.querySelector("[data-dl-primary]");
  var chips = Array.prototype.slice.call(document.querySelectorAll("[data-os]"));
  var note = document.querySelector("[data-dl-note]");
  if (!btn) return;
  var label = btn.querySelector("[data-dl-label]");
  var file = btn.querySelector("[data-dl-file]");
  var fileDefault = file.textContent;

  function mySystem() {
    var p = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent;
    if (/win/i.test(p)) return "win";
    if (/linux|x11/i.test(p) && !/android/i.test(navigator.userAgent)) return "linux";
    return "mac";
  }
  function pick(os) {
    label.textContent = btn.getAttribute("data-tpl").replace("{os}", names[os]);
    var f = files[os];
    btn.href = f ? f.url : "https://github.com/curva3d/curva/releases";
    file.textContent = f ? f.name : fileDefault;
    if (note) note.textContent = note.getAttribute("data-note-" + os).toUpperCase() + " · " + note.getAttribute("data-free").toUpperCase();
    chips.forEach(function (c) { c.setAttribute("aria-pressed", String(c.getAttribute("data-os") === os)); });
  }
  chips.forEach(function (c) {
    c.setAttribute("role", "button");
    c.addEventListener("click", function (e) { e.preventDefault(); pick(c.getAttribute("data-os")); });
  });
  var current = mySystem();
  pick(current);
  chips.forEach(function (c) { c.addEventListener("click", function () { current = c.getAttribute("data-os"); }); });

  fetch(API, { headers: { Accept: "application/vnd.github+json" } })
    .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(function (list) {
      var rel = (list || []).filter(function (r) { return !r.draft; })[0];
      if (!rel) return;
      (rel.assets || []).forEach(function (a) {
        Object.keys(match).forEach(function (os) {
          if (match[os](a.name) && !files[os]) files[os] = { url: a.browser_download_url, name: a.name };
        });
      });
      pick(current);
    })
    .catch(function () { /* keep the Releases page links */ });
})();
