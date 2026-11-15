// Fills in the latest release from GitHub: the version number and a direct link per system.
// Without JavaScript, or if GitHub doesn't answer, every link still goes to the Releases page.
(function () {
  var API = "https://api.github.com/repos/curva3d/curva/releases/latest";
  var match = {
    win: function (n) { return /\.exe$/i.test(n); },
    linux: function (n) { return /\.appimage$/i.test(n); },
    mac: function (n) { return /\.dmg$/i.test(n); }
  };
  function mySystem() {
    var p = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent;
    if (/win/i.test(p)) return "win";
    if (/mac|iphone|ipad/i.test(p)) return "mac";
    if (/linux|x11/i.test(p)) return "linux";
    return null;
  }
  fetch(API, { headers: { Accept: "application/vnd.github+json" } })
    .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(function (rel) {
      var version = (rel.tag_name || "").replace(/^v/, "");
      document.querySelectorAll("[data-version]").forEach(function (el) { if (version) el.textContent = version; });
      var assets = rel.assets || [];
      Object.keys(match).forEach(function (os) {
        var a = assets.filter(function (x) { return match[os](x.name); })[0];
        if (!a) return;
        document.querySelectorAll('[data-dl="' + os + '"]').forEach(function (el) { el.href = a.browser_download_url; });
        var mine = mySystem();
        if (mine === os) {
          document.querySelectorAll("[data-dl-primary]").forEach(function (el) {
            el.href = a.browser_download_url;
            var label = el.getAttribute("data-label-" + os);
            var span = el.querySelector("span");
            if (label && span) span.textContent = label;
          });
        }
      });
    })
    .catch(function () { /* keep the Releases page links */ });
})();
