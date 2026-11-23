// Curva's UI kit: glass panels, cards and buttons lean toward the cursor with a soft light where it
// is (styles: .wt in kit.css). Shared by the website and the docs. Off on touch screens and when
// the system asks for less motion.
(function () {
  if (!window.matchMedia("(hover: hover)").matches || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  var PANELS = ".info, .dl, .figure, .starlight-aside, .card, .sl-link-card, .pagination-links a";
  var BUTTONS = ".dl-btn, .go, .os, .theme, .sl-link-button";
  document.querySelectorAll(PANELS).forEach(function (el) { el.classList.add("wt"); });
  document.querySelectorAll(BUTTONS).forEach(function (el) { el.classList.add("wt", "btn"); });
  document.querySelectorAll(".wt").forEach(function (el) {
    el.addEventListener("pointermove", function (e) {
      var r = el.getBoundingClientRect(), px = (e.clientX - r.left) / r.width, py = (e.clientY - r.top) / r.height;
      el.style.setProperty("--px", (px * 2 - 1).toFixed(3)); el.style.setProperty("--py", (py * 2 - 1).toFixed(3));
      el.style.setProperty("--pd", "1"); el.style.setProperty("--gx", (px * 100).toFixed(1) + "%"); el.style.setProperty("--gy", (py * 100).toFixed(1) + "%");
    });
    el.addEventListener("pointerleave", function () { el.style.setProperty("--pd", "0"); el.style.setProperty("--px", "0"); el.style.setProperty("--py", "0"); });
  });
})();
