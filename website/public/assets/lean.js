// Docs: cards and buttons lean toward the cursor with a soft light where it is, as on the website.
(function () {
  if (!window.matchMedia("(hover: hover)").matches || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  document.querySelectorAll(".starlight-aside, .card, .sl-link-card, .pagination-links a").forEach(function (el) { el.classList.add("wt"); });
  document.querySelectorAll(".sl-link-button").forEach(function (el) { el.classList.add("wt", "btn"); });
  document.querySelectorAll(".wt").forEach(function (el) {
    el.addEventListener("pointermove", function (e) {
      var r = el.getBoundingClientRect(), px = (e.clientX - r.left) / r.width, py = (e.clientY - r.top) / r.height;
      el.style.setProperty("--px", (px * 2 - 1).toFixed(3)); el.style.setProperty("--py", (py * 2 - 1).toFixed(3));
      el.style.setProperty("--pd", "1"); el.style.setProperty("--gx", (px * 100).toFixed(1) + "%"); el.style.setProperty("--gy", (py * 100).toFixed(1) + "%");
    });
    el.addEventListener("pointerleave", function () { el.style.setProperty("--pd", "0"); el.style.setProperty("--px", "0"); el.style.setProperty("--py", "0"); });
  });
})();
