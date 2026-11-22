// Light or dark. Runs before the page is drawn so there is no flash of the wrong one.
// Follows the visitor's system unless they chose with the button; the choice is kept under the
// same name the docs use, so the home pages and the docs agree.
(function () {
  var root = document.documentElement;
  var KEY = "starlight-theme";
  var mq = window.matchMedia("(prefers-color-scheme: dark)");
  var choice = null;
  try { choice = localStorage.getItem(KEY); } catch (e) {}
  function apply() {
    var t = choice === "dark" || choice === "light" ? choice : mq.matches ? "dark" : "light";
    root.setAttribute("data-theme", t);
    var metas = document.querySelectorAll('meta[name="theme-color"]');
    for (var i = 0; i < metas.length; i++) { metas[i].removeAttribute("media"); metas[i].content = t === "dark" ? "#0b0b0b" : "#efefef"; }
  }
  root.classList.add("js");
  apply();
  if (mq.addEventListener) mq.addEventListener("change", apply);
  document.addEventListener("DOMContentLoaded", apply);
  window.curvaTheme = {
    toggle: function () {
      var next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
      // Choosing what the system already says goes back to following the system.
      choice = next === (mq.matches ? "dark" : "light") ? null : next;
      try { if (choice) localStorage.setItem(KEY, choice); else localStorage.removeItem(KEY); } catch (e) {}
      apply();
    }
  };
})();
