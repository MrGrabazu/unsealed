// The showcase page: the game's logo (inlined, so it uses the page's Cinzel), the language
// switch, and the download button pointing straight at the latest installer.
(function () {
  var REPO = "MrGrabazu/unsealed";

  // The logo, as the game draws it.
  fetch("logo.svg")
    .then(function (r) { return r.ok ? r.text() : Promise.reject(); })
    .then(function (svg) { document.getElementById("logo").innerHTML = svg; })
    .catch(function () {});

  // French or English, remembered.
  document.querySelectorAll("[data-set-lang]").forEach(function (button) {
    button.addEventListener("click", function () {
      var lang = button.dataset.setLang;
      document.documentElement.lang = lang;
      document.documentElement.dataset.lang = lang;
      try { localStorage.setItem("unsealed-site-lang", lang); } catch (e) {}
    });
  });

  // Each installer of a release, by the end of its file name.
  var ASSETS = {
    windows: /-setup\.exe$/,
    macos: /\.dmg$/,
    appimage: /\.AppImage$/,
    deb: /\.deb$/,
    rpm: /\.rpm$/,
  };
  var NAMES = { windows: "Windows", macos: "macOS", appimage: "Linux" };

  // The visitor's platform: the main button offers its installer.
  var ua = navigator.userAgent || "";
  var os = /Mac/.test(ua) && !/iPhone|iPad/.test(ua) ? "macos" : /Linux/.test(ua) && !/Android/.test(ua) ? "appimage" : "windows";
  document.querySelectorAll("[data-os-name]").forEach(function (b) { b.textContent = NAMES[os]; });

  // The latest release's installers: direct links, and the main one's version and size.
  // Without a release (or offline), every link keeps pointing at the releases page.
  fetch("https://api.github.com/repos/" + REPO + "/releases/latest")
    .then(function (r) { return r.ok ? r.json() : Promise.reject(); })
    .then(function (release) {
      var assets = release.assets || [];
      var find = function (kind) { return assets.find(function (a) { return ASSETS[kind].test(a.name); }); };
      document.querySelectorAll("[data-asset]").forEach(function (link) {
        var asset = find(link.dataset.asset);
        if (asset) link.href = asset.browser_download_url;
        else link.hidden = true;
      });
      var main = find(os) || find("windows");
      if (!main) return;
      document.getElementById("download").href = main.browser_download_url;
      var size = (main.size / 1048576).toFixed(1);
      var version = release.tag_name.replace(/^v/, "");
      document.getElementById("release").innerHTML =
        '<span lang="fr">Version ' + version + " · " + size.replace(".", ",") + " Mo · gratuit · mises à jour intégrées</span>" +
        '<span lang="en">Version ' + version + " · " + size + " MB · free · built-in updates</span>";
    })
    .catch(function () {});

  // A little gold dust, as in the game (not for those who turn motion off).
  if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    var motes = document.querySelector(".motes");
    for (var i = 0; i < 28; i++) {
      var mote = document.createElement("i");
      mote.style.left = Math.random() * 100 + "%";
      mote.style.top = Math.random() * 100 + "%";
      mote.style.animationDelay = -Math.random() * 20 + "s";
      mote.style.animationDuration = 14 + Math.random() * 14 + "s";
      motes.appendChild(mote);
    }
  }
})();
